//! Structure builder — port of services/documents/builder.py.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use crate::blocks::{Block, BlockType};
use crate::{pystr, textstats};

static REFERENCES_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^[^\p{L}\p{N}_]*(?:\d+(?:\.\d+)*\.?[\s\x1c-\x1f]*)?(?:references|bibliography|works[\s\x1c-\x1f]+cited|sources|literature[\s\x1c-\x1f]+cited)[^\p{L}\p{N}_]*$",
    )
    .unwrap()
});
static NOTES_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[^\p{L}\p{N}_]*(?:\d+(?:\.\d+)*\.?[\s\x1c-\x1f]*)?(?:notes|footnotes|endnotes)[^\p{L}\p{N}_]*$")
        .unwrap()
});

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub headings: u64,
    pub paragraphs: u64,
    pub lists: u64,
    pub list_items: u64,
    pub tables: u64,
    pub figures: u64,
    pub code_blocks: u64,
    pub blockquotes: u64,
    pub footnotes: u64,
    pub references: u64,
    pub links: u64,
    pub sentences: u64,
    pub words: u64,
}

impl std::ops::AddAssign for Counts {
    fn add_assign(&mut self, o: Self) {
        self.headings += o.headings;
        self.paragraphs += o.paragraphs;
        self.lists += o.lists;
        self.list_items += o.list_items;
        self.tables += o.tables;
        self.figures += o.figures;
        self.code_blocks += o.code_blocks;
        self.blockquotes += o.blockquotes;
        self.footnotes += o.footnotes;
        self.references += o.references;
        self.links += o.links;
        self.sentences += o.sentences;
        self.words += o.words;
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Totals {
    #[serde(flatten)]
    pub counts: Counts,
    pub sections: u64,
    pub max_depth: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SectionNode {
    pub kind: &'static str,
    pub level: u8,
    pub index: u64,
    pub path: String,
    pub counts: Counts,
    pub totals: Counts,
    pub children: Vec<SectionNode>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Structure {
    pub totals: Totals,
    pub sections: Vec<SectionNode>,
}

struct Section {
    level: u8,
    heading: Option<Block>,
    blocks: Vec<Block>,
    children: Vec<usize>,
}

// ── Tree assembly ──────────────────────────────────────────────────────────────

fn assemble(blocks: Vec<Block>) -> (Vec<Section>, Vec<usize>) {
    let mut arena: Vec<Section> = Vec::new();
    let mut roots: Vec<usize> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for block in blocks {
        if block.kind == BlockType::Heading {
            let level = block.level.unwrap_or(1);
            while stack.last().is_some_and(|&top| arena[top].level >= level) {
                stack.pop();
            }
            let id = arena.len();
            arena.push(Section {
                level,
                heading: Some(block),
                blocks: vec![],
                children: vec![],
            });
            match stack.last() {
                Some(&parent) => arena[parent].children.push(id),
                None => roots.push(id),
            }
            stack.push(id);
        } else if let Some(&top) = stack.last() {
            arena[top].blocks.push(block);
        } else {
            if roots.first().is_none_or(|&r| arena[r].heading.is_some()) {
                arena.push(Section {
                    level: 0,
                    heading: None,
                    blocks: vec![],
                    children: vec![],
                });
                roots.insert(0, arena.len() - 1);
            }
            arena[roots[0]].blocks.push(block);
        }
    }
    (arena, roots)
}

fn retype(blocks: &mut [Block], target: BlockType) {
    for block in blocks {
        if matches!(block.kind, BlockType::Paragraph | BlockType::ListItem) {
            block.kind = target;
        }
        if matches!(block.kind, BlockType::List | BlockType::Blockquote) || block.kind == target {
            retype(&mut block.children, target);
        }
    }
}

fn classify(arena: &mut [Section], ids: &[usize], inherited: Option<BlockType>) {
    for &id in ids {
        let mut target = inherited;
        if let Some(h) = &arena[id].heading {
            let title = pystr::strip(&h.text);
            if REFERENCES_HEADING.is_match(title) {
                target = Some(BlockType::Reference);
            } else if NOTES_HEADING.is_match(title) {
                target = Some(BlockType::Footnote);
            }
        }
        if let Some(t) = target {
            retype(&mut arena[id].blocks, t);
        }
        let children = arena[id].children.clone();
        classify(arena, &children, target);
    }
}

// ── Counting ───────────────────────────────────────────────────────────────────

fn tally(block: &Block, acc: &mut Counts) {
    match block.kind {
        BlockType::Heading => acc.headings += 1,
        BlockType::Paragraph => acc.paragraphs += 1,
        BlockType::List => acc.lists += 1,
        BlockType::ListItem => acc.list_items += 1,
        BlockType::Table => acc.tables += 1,
        BlockType::Figure => acc.figures += 1,
        BlockType::CodeBlock => acc.code_blocks += 1,
        BlockType::Blockquote => acc.blockquotes += 1,
        BlockType::Footnote => acc.footnotes += 1,
        BlockType::Reference => acc.references += 1,
        BlockType::Link => acc.links += 1,
    }
    let prose = matches!(
        block.kind,
        BlockType::Paragraph | BlockType::ListItem | BlockType::Footnote
    );
    if prose && !pystr::strip(&block.text).is_empty() {
        acc.sentences += textstats::count_sentences(&block.text) as u64;
        acc.words += textstats::count_words(&block.text) as u64;
    }
    for child in &block.children {
        tally(child, acc);
    }
}

pub fn count_blocks<'a>(blocks: impl IntoIterator<Item = &'a Block>) -> Counts {
    let mut acc = Counts::default();
    for b in blocks {
        tally(b, &mut acc);
    }
    acc
}

fn to_node(arena: &[Section], id: usize, index: u64, parent_path: &str) -> (SectionNode, u64, u64) {
    let s = &arena[id];
    let own = count_blocks(s.heading.iter().chain(s.blocks.iter()));
    let path = if s.heading.is_none() {
        "0".to_string()
    } else {
        format!("{parent_path}{index}")
    };
    let mut totals = own;
    let mut children = Vec::with_capacity(s.children.len());
    let (mut n_sections, mut depth) = (1u64, 1u64);
    let child_prefix = format!("{path}.");
    for (i, &child) in s.children.iter().enumerate() {
        let (node, n, d) = to_node(arena, child, i as u64 + 1, &child_prefix);
        totals += node.totals;
        children.push(node);
        n_sections += n;
        depth = depth.max(d + 1);
    }
    let node = SectionNode {
        kind: if s.heading.is_none() {
            "preamble"
        } else {
            "section"
        },
        level: s.level,
        index,
        path,
        counts: own,
        totals,
        children,
    };
    (node, n_sections, depth)
}

pub fn build_structure(blocks: Vec<Block>) -> Structure {
    let (mut arena, roots) = assemble(blocks);
    classify(&mut arena, &roots, None);
    let mut sections = Vec::with_capacity(roots.len());
    let mut totals = Counts::default();
    let (mut n_sections, mut max_depth, mut ordinal) = (0u64, 0u64, 0u64);
    for &id in &roots {
        if arena[id].heading.is_some() {
            ordinal += 1;
        }
        let (node, n, d) = to_node(&arena, id, ordinal, "");
        totals += node.totals;
        sections.push(node);
        n_sections += n;
        max_depth = max_depth.max(d);
    }
    Structure {
        totals: Totals {
            counts: totals,
            sections: n_sections,
            max_depth,
        },
        sections,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::heading;

    fn p() -> Block {
        Block::with_text(BlockType::Paragraph, "One sentence here.")
    }

    fn outline(nodes: &[SectionNode], out: &mut Vec<(u8, String)>) {
        for n in nodes {
            out.push((n.level, n.path.clone()));
            outline(&n.children, out);
        }
    }

    #[test]
    fn skipped_levels_nest_directly() {
        let s = build_structure(vec![
            heading(1, "A"),
            heading(3, "x"),
            p(),
            heading(2, "B"),
            heading(4, "y"),
        ]);
        let mut o = vec![];
        outline(&s.sections, &mut o);
        let want: Vec<(u8, String)> = vec![
            (1, "1".into()),
            (3, "1.1".into()),
            (2, "1.2".into()),
            (4, "1.2.1".into()),
        ];
        assert_eq!(o, want);
        assert_eq!(s.totals.max_depth, 3);
    }

    #[test]
    fn preamble_and_empty() {
        let s = build_structure(vec![p(), heading(1, "A")]);
        assert_eq!(s.sections[0].kind, "preamble");
        assert_eq!(s.sections[0].index, 0);
        assert_eq!(s.sections[1].index, 1);
        assert_eq!(build_structure(vec![]).totals, Totals::default());
    }

    #[test]
    fn references_retyped() {
        let s = build_structure(vec![heading(2, "3. References"), p()]);
        assert_eq!(
            (s.totals.counts.references, s.totals.counts.paragraphs),
            (1, 0)
        );
    }
}
