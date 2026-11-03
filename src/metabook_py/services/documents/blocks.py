"""
Common block tree that every format adapter produces.

Blocks hold text internally (for counting and for classifying sections such as
"References"), but nothing in this tree is ever serialized — the structure
builder reduces it to counts.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from enum import StrEnum


class BlockType(StrEnum):
    HEADING = "heading"
    PARAGRAPH = "paragraph"
    LIST = "list"
    LIST_ITEM = "list_item"
    TABLE = "table"
    FIGURE = "figure"
    CODE_BLOCK = "code_block"
    BLOCKQUOTE = "blockquote"
    FOOTNOTE = "footnote"
    REFERENCE = "reference"
    LINK = "link"


@dataclass
class Block:
    type: BlockType
    text: str = ""  # prose used for counting; empty for containers
    level: int | None = None  # heading level 1–6
    children: list[Block] = field(default_factory=list)

    def add_links(self, n: int) -> Block:
        self.children.extend(Block(BlockType.LINK) for _ in range(n))
        return self


def heading(level: int, text: str) -> Block:
    return Block(BlockType.HEADING, text=text, level=max(1, min(6, level)))


# ── Shared inline helpers (text, markdown, pdf) ────────────────────────────────

BARE_URL = re.compile(r"\bhttps?://[^\s<>()\[\]\"']+", re.IGNORECASE)
FIGURE_CAPTION = re.compile(r"^(?:Figure|Fig\.)\s*\d+", re.IGNORECASE)

# Bullets: -, *, +, •, ‣, ◦, ▪, –   Ordered: 1. 1) a. a) iv.
LIST_MARKER = re.compile(r"^(\s*)(?:[-*+•‣◦▪–]|\d{1,3}[.)]|[a-z][.)])\s+")
