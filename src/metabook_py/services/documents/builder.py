"""
Structure builder: block list → section tree with counts.

Rules (docs/single-document-schema.md):
- Top-level headings open sections; a heading closes open sections of the same
  or deeper level and nests under the nearest shallower one (skipped levels
  nest directly).
- Content before the first heading goes into a level-0 preamble, created only
  when such content exists.
- Paragraphs / list items under a References-like heading become references;
  under a Notes-like heading they become footnotes.
- Sentences and words come from services/textstats.py (shared with books).
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field

from metabook_py.models.document import Counts, DocumentTotals, SectionNode
from metabook_py.services.documents.blocks import Block, BlockType
from metabook_py.services.textstats import count_sentences, count_words

_REFERENCES_HEADING = re.compile(
    r"^\W*(?:\d+(?:\.\d+)*\.?\s*)?(?:references|bibliography|works\s+cited|sources|"
    r"literature\s+cited)\W*$",
    re.IGNORECASE,
)
_NOTES_HEADING = re.compile(
    r"^\W*(?:\d+(?:\.\d+)*\.?\s*)?(?:notes|footnotes|endnotes)\W*$", re.IGNORECASE
)


@dataclass
class _Section:
    level: int
    heading: Block | None
    blocks: list[Block] = field(default_factory=list)
    children: list[_Section] = field(default_factory=list)


# ── Tree assembly ──────────────────────────────────────────────────────────────


def _assemble(blocks: list[Block]) -> list[_Section]:
    roots: list[_Section] = []
    stack: list[_Section] = []
    for block in blocks:
        if block.type is BlockType.HEADING:
            level = block.level or 1
            while stack and stack[-1].level >= level:
                stack.pop()
            section = _Section(level=level, heading=block)
            (stack[-1].children if stack else roots).append(section)
            stack.append(section)
        elif stack:
            stack[-1].blocks.append(block)
        else:
            if not roots or roots[0].heading is not None:
                roots.insert(0, _Section(level=0, heading=None))
            roots[0].blocks.append(block)
    return roots


def _retype(blocks: list[Block], target: BlockType) -> None:
    for block in blocks:
        if block.type in (BlockType.PARAGRAPH, BlockType.LIST_ITEM):
            block.type = target
        if block.type in (BlockType.LIST, BlockType.BLOCKQUOTE, target):
            _retype(block.children, target)


def _classify(sections: list[_Section], inherited: BlockType | None = None) -> None:
    for section in sections:
        target = inherited
        if section.heading is not None:
            title = section.heading.text.strip()
            if _REFERENCES_HEADING.match(title):
                target = BlockType.REFERENCE
            elif _NOTES_HEADING.match(title):
                target = BlockType.FOOTNOTE
        if target is not None:
            _retype(section.blocks, target)
        _classify(section.children, target)


# ── Counting ───────────────────────────────────────────────────────────────────

_COUNTER_FIELD = {
    BlockType.HEADING: "headings",
    BlockType.PARAGRAPH: "paragraphs",
    BlockType.LIST: "lists",
    BlockType.LIST_ITEM: "list_items",
    BlockType.TABLE: "tables",
    BlockType.FIGURE: "figures",
    BlockType.CODE_BLOCK: "code_blocks",
    BlockType.BLOCKQUOTE: "blockquotes",
    BlockType.FOOTNOTE: "footnotes",
    BlockType.REFERENCE: "references",
    BlockType.LINK: "links",
}
_PROSE = frozenset({BlockType.PARAGRAPH, BlockType.LIST_ITEM, BlockType.FOOTNOTE})


def _tally(block: Block, acc: dict[str, int]) -> None:
    acc[_COUNTER_FIELD[block.type]] += 1
    if block.type in _PROSE and block.text.strip():
        acc["sentences"] += count_sentences(block.text)
        acc["words"] += count_words(block.text)
    for child in block.children:
        _tally(child, acc)


def count_blocks(blocks: list[Block]) -> Counts:
    acc = dict.fromkeys(Counts.model_fields, 0)
    for block in blocks:
        _tally(block, acc)
    return Counts(**acc)


def _to_node(section: _Section, index: int, parent_path: str) -> tuple[SectionNode, int, int]:
    """Return (node, section_count, depth) for *section* and its subtree."""
    own = count_blocks(([section.heading] if section.heading else []) + section.blocks)
    path = "0" if section.heading is None else f"{parent_path}{index}"
    children: list[SectionNode] = []
    totals = own
    n_sections, depth = 1, 1
    for i, child in enumerate(section.children):
        node, n, d = _to_node(child, i + 1, f"{path}.")
        children.append(node)
        totals = totals + node.totals
        n_sections += n
        depth = max(depth, d + 1)
    node = SectionNode(
        kind="preamble" if section.heading is None else "section",
        level=section.level,
        index=index,
        path=path,
        counts=own,
        totals=totals,
        children=children,
    )
    return node, n_sections, depth


# ── Public API ─────────────────────────────────────────────────────────────────


def build_document_structure(blocks: list[Block]) -> tuple[list[SectionNode], DocumentTotals]:
    sections = _assemble(blocks)
    _classify(sections)
    nodes: list[SectionNode] = []
    totals = Counts()
    n_sections = max_depth = 0
    # The preamble is index 0; headed sections are numbered from 1.
    ordinal = 0
    for section in sections:
        if section.heading is not None:
            ordinal += 1
        node, n, d = _to_node(section, ordinal, "")
        nodes.append(node)
        totals = totals + node.totals
        n_sections += n
        max_depth = max(max_depth, d)
    return nodes, DocumentTotals(**totals.model_dump(), sections=n_sections, max_depth=max_depth)
