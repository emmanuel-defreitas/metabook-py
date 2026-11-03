//! Common block tree — port of services/documents/blocks.py.

use std::sync::LazyLock;

use regex::Regex;

use crate::pystr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockType {
    Heading,
    Paragraph,
    List,
    ListItem,
    Table,
    Figure,
    CodeBlock,
    Blockquote,
    Footnote,
    Reference,
    Link,
}

impl BlockType {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "heading" => Self::Heading,
            "paragraph" => Self::Paragraph,
            "list" => Self::List,
            "list_item" => Self::ListItem,
            "table" => Self::Table,
            "figure" => Self::Figure,
            "code_block" => Self::CodeBlock,
            "blockquote" => Self::Blockquote,
            "footnote" => Self::Footnote,
            "reference" => Self::Reference,
            "link" => Self::Link,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Block {
    pub kind: BlockType,
    pub text: String,
    pub level: Option<u8>,
    pub children: Vec<Block>,
}

impl Block {
    pub fn new(kind: BlockType) -> Self {
        Self {
            kind,
            text: String::new(),
            level: None,
            children: Vec::new(),
        }
    }

    pub fn with_text(kind: BlockType, text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::new(kind)
        }
    }

    pub fn add_links(mut self, n: usize) -> Self {
        self.push_links(n);
        self
    }

    pub fn push_links(&mut self, n: usize) {
        self.children
            .extend((0..n).map(|_| Block::new(BlockType::Link)));
    }
}

pub fn heading(level: i64, text: impl Into<String>) -> Block {
    Block {
        level: Some(level.clamp(1, 6) as u8),
        ..Block::with_text(BlockType::Heading, text)
    }
}

// ── Shared inline helpers (text, markdown) ─────────────────────────────────────

// Python: (?i)\bhttps?://[^\s<>()\[\]"']+ — the leading \b uses Python's
// word class, so candidates are scanned position by position (count_bare_urls).
static BARE_URL_AT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)^https?://[^\s\x1c-\x1f<>()\[\]"']+"#).unwrap());

/// `len(BARE_URL.findall(text))`.
pub fn count_bare_urls(text: &str) -> usize {
    let mut n = 0;
    let mut next_free = 0;
    for (i, c) in text.char_indices() {
        if i < next_free || !matches!(c, 'h' | 'H') || !pystr::word_start(text, i) {
            continue;
        }
        if let Some(m) = BARE_URL_AT.find(&text[i..]) {
            n += 1;
            next_free = i + m.end();
        }
    }
    n
}
pub static FIGURE_CAPTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:Figure|Fig\.)[\s\x1c-\x1f]*\d+").unwrap());
pub static LIST_MARKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^([\s\x1c-\x1f]*)(?:[-*+\u{2022}\u{2023}\u{25e6}\u{25aa}\u{2013}]|\d{1,3}[.)]|[a-z][.)])[\s\x1c-\x1f]+",
    )
    .unwrap()
});
