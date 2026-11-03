//! Markdown adapter — port of services/documents/markdown_adapter.py.
//!
//! The Python patterns use a backreference (thematic breaks) and a negative
//! lookbehind (`(?<!!)` to skip images), neither of which the `regex` crate
//! supports; both are re-expressed below with identical matching behavior.

use std::sync::LazyLock;

use regex::{Captures, Regex};

use crate::blocks::{Block, BlockType, count_bare_urls, heading};
use crate::pystr;

macro_rules! re {
    ($name:ident, $pat:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($pat).unwrap());
    };
}

// `\s` below is spelled [\s\x1c-\x1f] to match Python's definition.
re!(FENCE, r"^[\s\x1c-\x1f]{0,3}(`{3,}|~{3,})");
re!(
    ATX,
    r"^[\s\x1c-\x1f]{0,3}(#{1,6})(?:[ \t]+(.*?))?(?:[ \t]+#+)?[ \t]*$"
);
re!(SETEXT, r"^[\s\x1c-\x1f]{0,3}(=+|-+)[ \t]*$");
// Python: ^\s{0,3}([-*_])(?:[ \t]*\1){2,}[ \t]*$
re!(
    HR,
    r"^[\s\x1c-\x1f]{0,3}(?:-(?:[ \t]*-){2,}|\*(?:[ \t]*\*){2,}|_(?:[ \t]*_){2,})[ \t]*$"
);
re!(QUOTE, r"^[\s\x1c-\x1f]{0,3}> ?");
re!(FOOTNOTE_DEF, r"^[\s\x1c-\x1f]{0,3}\[\^[^\]]+\]:[ \t]*(.*)$");
re!(
    LINK_DEF,
    r"^[\s\x1c-\x1f]{0,3}\[[^\]^][^\]]*\]:[\s\x1c-\x1f]*[^\s\x1c-\x1f]+"
);
re!(
    TABLE_SEP,
    r"^[\s\x1c-\x1f]*\|?[\s\x1c-\x1f]*:?-+:?[\s\x1c-\x1f]*(?:\|[\s\x1c-\x1f]*:?-+:?[\s\x1c-\x1f]*)*\|?[\s\x1c-\x1f]*$"
);
re!(
    LIST,
    r"^([\s\x1c-\x1f]*)([-*+]|\d{1,9}[.)])(?:[ \t]+(.*))?$"
);
re!(
    IMAGE_LINE,
    r"^[\s\x1c-\x1f]*!\[[^\]]*\]\([^)]*\)[\s\x1c-\x1f]*$"
);
re!(INDENTED, r"^(?: {4}|\t)");

// Inline syntax
re!(IMAGE, r"!\[([^\]]*)\]\([^)]*\)");
// Anchored forms of Python's (?<!!)\[…\]… — scanned by `scan_not_after_bang`.
re!(INLINE_LINK_AT, r"^\[([^\]]*)\]\([^)]*\)");
re!(REF_LINK_AT, r"^\[([^\]]+)\]\[[^\]]*\]");
re!(
    AUTOLINK,
    r"<[a-zA-Z][a-zA-Z0-9+.-]{1,31}:[^\s\x1c-\x1f<>]*>"
);
re!(FOOTNOTE_REF, r"\[\^[^\]]+\]");

/// Python `re.sub`/`findall` for `(?<!!)` + anchored pattern: try each `[`
/// not preceded by `!`, leftmost first, non-overlapping. Returns the text with
/// each match replaced by `repl(caps)` and the match count.
fn scan_not_after_bang(
    text: &str,
    anchored: &Regex,
    repl: impl Fn(&Captures) -> String,
) -> (String, usize) {
    let mut out = String::with_capacity(text.len());
    let mut n = 0;
    let mut i = 0;
    let bytes = text.as_bytes();
    while i < text.len() {
        if bytes[i] == b'['
            && (i == 0 || bytes[i - 1] != b'!')
            && let Some(caps) = anchored.captures(&text[i..])
        {
            let end = i + caps.get(0).unwrap().end();
            out.push_str(&repl(&caps));
            n += 1;
            i = end;
            continue;
        }
        let ch = text[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    (out, n)
}

pub fn count_links(raw: &str) -> usize {
    let (raw, a) = scan_not_after_bang(raw, &INLINE_LINK_AT, |_| " ".into());
    let (raw, b) = scan_not_after_bang(&raw, &REF_LINK_AT, |_| " ".into());
    let c = AUTOLINK.find_iter(&raw).count();
    let raw = AUTOLINK.replace_all(&raw, " ");
    a + b + c + count_bare_urls(&IMAGE.replace_all(&raw, " "))
}

pub fn strip_inline(raw: &str) -> String {
    let text = IMAGE.replace_all(raw, " ");
    let (text, _) = scan_not_after_bang(&text, &INLINE_LINK_AT, |c| c[1].to_string());
    let (text, _) = scan_not_after_bang(&text, &REF_LINK_AT, |c| c[1].to_string());
    let text = AUTOLINK.replace_all(&text, " ");
    let text = FOOTNOTE_REF.replace_all(&text, "");
    pystr::collapse_ws(&text)
}

fn prose(kind: BlockType, raw: &str) -> Block {
    Block::with_text(kind, strip_inline(raw)).add_links(count_links(raw))
}

fn md_heading(level: i64, raw: &str) -> Block {
    heading(level, strip_inline(raw)).add_links(count_links(raw))
}

fn starts_block(line: &str) -> bool {
    FENCE.is_match(line)
        || ATX.is_match(line)
        || QUOTE.is_match(line)
        || HR.is_match(line)
        || LIST.is_match(line)
}

/// Strip every leading blockquote marker.
fn unquote(line: &str) -> &str {
    let mut line = line;
    while let Some(m) = QUOTE.find(line) {
        line = &line[m.end()..];
    }
    line
}

/// Whether a quoted line (outer marker already stripped) can be lazily continued.
fn is_paragraph_text(line: &str) -> bool {
    let line = unquote(line);
    !blank(line)
        && !(ATX.is_match(line)
            || FENCE.is_match(line)
            || HR.is_match(line)
            || SETEXT.is_match(line))
}

/// The blockquote at `start`, markers stripped, and the index just past it.
///
/// An unquoted line stays in the quote only as lazy paragraph continuation:
/// the previous line must be open paragraph text (not a heading, rule or code)
/// and the line itself must not start a new block.
fn quote_lines(lines: &[String], start: usize) -> (Vec<String>, usize) {
    let mut inner: Vec<String> = Vec::new();
    let mut in_fence = false;
    let n = lines.len();
    let mut i = start;
    while i < n && !blank(&lines[i]) {
        let line = &lines[i];
        if let Some(m) = QUOTE.find(line) {
            let content = &line[m.end()..];
            if FENCE.is_match(unquote(content)) {
                in_fence = !in_fence;
            }
            inner.push(content.to_string());
        } else if !in_fence
            && inner.last().is_some_and(|prev| is_paragraph_text(prev))
            && !starts_block(line)
        {
            inner.push(line.clone());
        } else {
            break;
        }
        i += 1;
    }
    (inner, i)
}

fn indented(line: &str) -> bool {
    line.starts_with(' ') || line.starts_with('\t')
}

fn blank(line: &str) -> bool {
    pystr::strip(line).is_empty()
}

fn strip_front_matter(lines: &[String]) -> &[String] {
    if lines.first().is_some_and(|l| pystr::strip(l) == "---") {
        for i in 1..lines.len() {
            let s = pystr::strip(&lines[i]);
            if s == "---" || s == "..." {
                return &lines[i + 1..];
            }
        }
    }
    lines
}

fn list_end(lines: &[String], start: usize) -> usize {
    let n = lines.len();
    let mut j = start + 1;
    while j < n {
        let line = &lines[j];
        if blank(line) {
            let nxt = lines.get(j + 1).map(String::as_str).unwrap_or("");
            if !blank(nxt) && (LIST.is_match(nxt) || indented(nxt)) {
                j += 1;
                continue;
            }
            break;
        }
        if LIST.is_match(line) || indented(line) {
            j += 1;
            continue;
        }
        if !blank(&lines[j - 1]) && !starts_block(line) {
            j += 1;
            continue;
        }
        break;
    }
    j
}

fn parse_list(lines: &[String]) -> Block {
    let first = LIST.captures(&lines[0]).unwrap();
    let base = pystr::len(&first[1]);
    let mut items: Vec<(usize, Vec<String>)> = Vec::new();
    for line in lines {
        let m = LIST.captures(line);
        match m {
            Some(m) if pystr::len(&m[1]) <= base + 1 => {
                let content_indent = pystr::len(&m[1]) + m[2].chars().count() + 1;
                let text = m.get(3).map_or("", |g| g.as_str()).to_string();
                items.push((content_indent, vec![text]));
            }
            _ => {
                if let Some((indent, body)) = items.last_mut() {
                    let leading = line.len() - line.trim_start_matches(' ').len();
                    body.push(line[(*indent).min(leading)..].to_string());
                }
            }
        }
    }
    let mut block = Block::new(BlockType::List);
    for (_, body) in items {
        let mut children = parse_lines(&body);
        let mut item = Block::new(BlockType::ListItem);
        if children
            .first()
            .is_some_and(|c| c.kind == BlockType::Paragraph)
        {
            let lead = children.remove(0);
            item.text = lead.text;
            item.children.extend(lead.children);
        }
        item.children.extend(children);
        block.children.push(item);
    }
    block
}

pub fn parse_lines(lines: &[String]) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let flush = |para: &mut Vec<String>, blocks: &mut Vec<Block>| {
        if !para.is_empty() {
            blocks.push(prose(BlockType::Paragraph, &para.join(" ")));
            para.clear();
        }
    };

    let n = lines.len();
    let mut i = 0;
    while i < n {
        let line = &lines[i];
        if blank(line) {
            flush(&mut para, &mut blocks);
            i += 1;
            continue;
        }

        if let Some(fence) = FENCE.captures(line) {
            flush(&mut para, &mut blocks);
            let marker = &fence[1];
            let ch = regex::escape(&marker[..1]);
            let close = Regex::new(&format!(
                r"^[\s\x1c-\x1f]{{0,3}}{ch}{{{},}}[\s\x1c-\x1f]*$",
                marker.len()
            ))
            .unwrap();
            i += 1;
            while i < n && !close.is_match(&lines[i]) {
                i += 1;
            }
            blocks.push(Block::new(BlockType::CodeBlock));
            i += 1;
            continue;
        }

        if let Some(atx) = ATX.captures(line) {
            flush(&mut para, &mut blocks);
            let raw = atx.get(2).map_or("", |g| g.as_str());
            blocks.push(md_heading(atx[1].len() as i64, raw));
            i += 1;
            continue;
        }

        if !para.is_empty()
            && let Some(setext) = SETEXT.captures(line)
        {
            let level = if setext[1].starts_with('=') { 1 } else { 2 };
            blocks.push(md_heading(level, &para.join(" ")));
            para.clear();
            i += 1;
            continue;
        }

        if HR.is_match(line) {
            flush(&mut para, &mut blocks);
            i += 1;
            continue;
        }

        if QUOTE.is_match(line) {
            flush(&mut para, &mut blocks);
            let (inner, end) = quote_lines(lines, i);
            i = end;
            let mut quote = Block::new(BlockType::Blockquote);
            quote.children = parse_lines(&inner);
            blocks.push(quote);
            continue;
        }

        if let Some(fn_def) = FOOTNOTE_DEF.captures(line) {
            flush(&mut para, &mut blocks);
            let mut raw = vec![fn_def[1].to_string()];
            i += 1;
            while i < n && !blank(&lines[i]) && INDENTED.is_match(&lines[i]) {
                raw.push(pystr::strip(&lines[i]).to_string());
                i += 1;
            }
            blocks.push(prose(BlockType::Footnote, &raw.join(" ")));
            continue;
        }

        if para.is_empty() && LINK_DEF.is_match(line) {
            i += 1;
            continue;
        }

        if line.contains('|')
            && i + 1 < n
            && TABLE_SEP.is_match(&lines[i + 1])
            && lines[i + 1].contains('-')
        {
            flush(&mut para, &mut blocks);
            let mut links = 0;
            while i < n && !blank(&lines[i]) && lines[i].contains('|') {
                links += count_links(&lines[i]);
                i += 1;
            }
            blocks.push(Block::new(BlockType::Table).add_links(links));
            continue;
        }

        // Indented code wins over list markers at the start of a block; after
        // paragraph text, an indented marker still opens a (nested) list.
        if para.is_empty() && INDENTED.is_match(line) {
            while i < n && (INDENTED.is_match(&lines[i]) || blank(&lines[i])) {
                i += 1;
            }
            blocks.push(Block::new(BlockType::CodeBlock));
            continue;
        }

        if LIST
            .captures(line)
            .is_some_and(|m| m.get(3).is_some_and(|g| !g.as_str().is_empty()))
        {
            flush(&mut para, &mut blocks);
            let end = list_end(lines, i);
            blocks.push(parse_list(&lines[i..end]));
            i = end;
            continue;
        }

        if para.is_empty() && IMAGE_LINE.is_match(line) {
            blocks.push(Block::new(BlockType::Figure));
            i += 1;
            continue;
        }

        para.push(pystr::strip(line).to_string());
        i += 1;
    }
    flush(&mut para, &mut blocks);
    blocks
}

pub fn parse_markdown(text: &str) -> Vec<Block> {
    let lines: Vec<String> = pystr::normalize_newlines(text)
        .split('\n')
        .map(String::from)
        .collect();
    parse_lines(strip_front_matter(&lines))
}
