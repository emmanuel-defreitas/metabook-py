//! Plain-text adapter — port of services/documents/text_adapter.py.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::blocks::{Block, BlockType, FIGURE_CAPTION, LIST_MARKER, count_bare_urls, heading};
use crate::pystr;

static SETEXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\s\x1c-\x1f]*(=+|-+)[\s\x1c-\x1f]*$").unwrap());
static NUMBERED_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{1,3}(?:\.\d{1,3}){0,5})\.?[\s\x1c-\x1f]+([A-Z][^\n]*)$").unwrap()
});
static FOOTNOTE_DEF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[\^[^\]]+\]:[\s\x1c-\x1f]*").unwrap());
static QUOTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\s\x1c-\x1f]*>[\s\x1c-\x1f]?").unwrap());
static HEADING_TERMINAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[.!?,;:]$").unwrap());

/// Blank-line separated chunks of non-empty lines (trailing spaces trimmed).
pub fn split_chunks(text: &str) -> Vec<Vec<String>> {
    let mut chunks = Vec::new();
    let mut current: Vec<String> = Vec::new();
    for line in pystr::normalize_newlines(text).split('\n') {
        if !pystr::strip(line).is_empty() {
            current.push(pystr::rstrip(line).to_string());
        } else if !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn count_urls(s: &str) -> usize {
    count_bare_urls(s)
}

fn paragraph(lines: &[String], kind: BlockType) -> Block {
    let text = lines
        .iter()
        .map(|l| pystr::strip(l))
        .collect::<Vec<_>>()
        .join(" ");
    let n = count_urls(&text);
    Block::with_text(kind, text).add_links(n)
}

fn heading_level(line: &str) -> Option<i64> {
    let line = pystr::strip(line);
    if pystr::len(line) > 80
        || pystr::split_ws(line).count() > 12
        || HEADING_TERMINAL.is_match(line)
    {
        return None;
    }
    if let Some(m) = NUMBERED_HEADING.captures(line) {
        return Some((m[1].matches('.').count() as i64 + 1).min(6));
    }
    let letters = line.chars().filter(|&c| pystr::is_alpha(c)).count();
    if letters >= 3 && line == line.to_uppercase() {
        return Some(1);
    }
    None
}

fn is_table(lines: &[String]) -> bool {
    if lines.len() < 2 {
        return false;
    }
    if lines.iter().all(|l| {
        let s = pystr::strip(l);
        s.trim_matches('|').contains('|') || s.starts_with('|')
    }) {
        return true;
    }
    let tabs: BTreeSet<usize> = lines.iter().map(|l| l.matches('\t').count()).collect();
    tabs.len() == 1 && *tabs.iter().next().unwrap() >= 1
}

/// Build a (possibly nested) list. Lists live in an arena so nested lists can
/// be attached to the item that owns them after the walk.
fn list(lines: &[String]) -> Block {
    struct Node {
        block: Block,
        owner: Vec<(usize, usize)>, // (item index, nested list id)
    }
    let first_indent = pystr::len(&LIST_MARKER.captures(&lines[0]).unwrap()[1]);
    let mut arena = vec![Node {
        block: Block::new(BlockType::List),
        owner: vec![],
    }];
    let mut stack: Vec<(usize, usize)> = vec![(first_indent, 0)]; // (indent, list id)
    for line in lines {
        let Some(m) = LIST_MARKER.captures(line) else {
            let top = stack.last().unwrap().1;
            if let Some(item) = arena[top].block.children.last_mut()
                && item.kind == BlockType::ListItem
            {
                let joined = format!("{} {}", item.text, pystr::strip(line));
                item.text = pystr::strip(&joined).to_string();
                item.push_links(count_urls(line));
            }
            continue;
        };
        let indent = pystr::len(&m[1]);
        while stack.len() > 1 && indent < stack.last().unwrap().0 {
            stack.pop();
        }
        let (top_indent, top) = *stack.last().unwrap();
        if indent > top_indent && !arena[top].block.children.is_empty() {
            let id = arena.len();
            let item_index = arena[top].block.children.len() - 1;
            arena.push(Node {
                block: Block::new(BlockType::List),
                owner: vec![],
            });
            arena[top].owner.push((item_index, id));
            stack.push((indent, id));
        }
        let top = stack.last().unwrap().1;
        let text = pystr::strip(&line[m.get(0).unwrap().end()..]).to_string();
        let n = count_urls(&text);
        arena[top]
            .block
            .children
            .push(Block::with_text(BlockType::ListItem, text).add_links(n));
    }
    // Attach nested lists bottom-up (children always have larger ids).
    for id in (0..arena.len()).rev() {
        let owners = std::mem::take(&mut arena[id].owner);
        for (item_index, child) in owners {
            let nested = std::mem::replace(&mut arena[child].block, Block::new(BlockType::List));
            arena[id].block.children[item_index].children.push(nested);
        }
    }
    std::mem::replace(&mut arena[0].block, Block::new(BlockType::List))
}

pub fn classify_chunk(lines: &[String], allow_headings: bool) -> Vec<Block> {
    if allow_headings
        && lines.len() >= 2
        && SETEXT.is_match(&lines[1])
        && !pystr::strip(&lines[0]).is_empty()
    {
        let level = if pystr::strip(&lines[1]).starts_with('=') {
            1
        } else {
            2
        };
        let title = pystr::strip(&lines[0]);
        let mut out = vec![heading(level, title).add_links(count_urls(title))];
        if lines.len() > 2 {
            out.extend(classify_chunk(&lines[2..], allow_headings));
        }
        return out;
    }

    let first = &lines[0];
    if lines.iter().all(|l| QUOTE.is_match(l)) {
        let inner = lines
            .iter()
            .map(|l| QUOTE.replace(l, "").into_owned())
            .collect::<Vec<_>>();
        let mut quote = Block::new(BlockType::Blockquote);
        quote.children = parse_text(&inner.join("\n"));
        return vec![quote];
    }
    if FOOTNOTE_DEF.is_match(first) {
        let mut body = vec![FOOTNOTE_DEF.replace(first, "").into_owned()];
        body.extend(lines[1..].iter().cloned());
        return vec![paragraph(&body, BlockType::Footnote)];
    }
    if LIST_MARKER.is_match(first)
        && !(allow_headings && lines.len() == 1 && heading_level(first).is_some())
    {
        return vec![list(lines)];
    }
    if is_table(lines) {
        let links = lines.iter().map(|l| count_urls(l)).sum();
        return vec![Block::new(BlockType::Table).add_links(links)];
    }
    if FIGURE_CAPTION.is_match(pystr::strip(first)) {
        return vec![Block::new(BlockType::Figure)];
    }
    if allow_headings
        && lines.len() == 1
        && let Some(level) = heading_level(first)
    {
        return vec![heading(level, pystr::strip(first)).add_links(count_urls(first))];
    }
    vec![paragraph(lines, BlockType::Paragraph)]
}

pub fn parse_text(text: &str) -> Vec<Block> {
    split_chunks(text)
        .iter()
        .flat_map(|c| classify_chunk(c, true))
        .collect()
}
