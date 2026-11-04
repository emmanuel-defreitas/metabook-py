"""
Plain-text adapter.

Blank-line separated chunks are classified as: setext heading, numbered or
ALL-CAPS heading, blockquote (">"), list, pipe/tab table, figure caption,
footnote definition ("[^1]: …"), or paragraph. Bare http(s) URLs are links.
"""

from __future__ import annotations

import re

from metabook_py.services.documents.blocks import (
    BARE_URL,
    FIGURE_CAPTION,
    LIST_MARKER,
    Block,
    BlockType,
    heading,
)

_SETEXT = re.compile(r"^\s*(=+|-+)\s*$")
_NUMBERED_HEADING = re.compile(r"^(\d{1,3}(?:\.\d{1,3}){0,5})\.?\s+([A-Z][^\n]*)$")
_FOOTNOTE_DEF = re.compile(r"^\[\^[^\]]+\]:\s*")
_QUOTE = re.compile(r"^\s*>\s?")
_HEADING_TERMINAL = re.compile(r"[.!?,;:]$")


def split_chunks(text: str) -> list[list[str]]:
    """Blank-line separated chunks of non-empty lines (trailing spaces trimmed)."""
    chunks: list[list[str]] = []
    current: list[str] = []
    for line in text.replace("\r\n", "\n").replace("\r", "\n").split("\n"):
        if line.strip():
            current.append(line.rstrip())
        elif current:
            chunks.append(current)
            current = []
    if current:
        chunks.append(current)
    return chunks


def _paragraph(lines: list[str], kind: BlockType = BlockType.PARAGRAPH) -> Block:
    text = " ".join(line.strip() for line in lines)
    return Block(kind, text=text).add_links(len(BARE_URL.findall(text)))


def _heading_level(line: str) -> int | None:
    """Level for a single-line chunk that reads like a heading, else None."""
    line = line.strip()
    if len(line) > 80 or len(line.split()) > 12 or _HEADING_TERMINAL.search(line):
        return None
    m = _NUMBERED_HEADING.match(line)
    if m:
        return min(6, m.group(1).count(".") + 1)
    letters = [c for c in line if c.isalpha()]
    if len(letters) >= 3 and line == line.upper():
        return 1
    return None


def _is_table(lines: list[str]) -> bool:
    if len(lines) < 2:
        return False
    if all("|" in line.strip().strip("|") or line.strip().startswith("|") for line in lines):
        return True
    tabs = {line.count("\t") for line in lines}
    return len(tabs) == 1 and tabs.pop() >= 1


def _list(lines: list[str]) -> Block:
    """Build a (possibly nested) list from marker-led lines."""
    root = Block(BlockType.LIST)
    # Stack of (indent, list_block); items append to the innermost list.
    stack: list[tuple[int, Block]] = [(len(LIST_MARKER.match(lines[0]).group(1)), root)]  # type: ignore[union-attr]
    for line in lines:
        m = LIST_MARKER.match(line)
        if m is None:
            # Continuation of the previous item.
            item = stack[-1][1].children[-1] if stack[-1][1].children else None
            if item is not None and item.type is BlockType.LIST_ITEM:
                item.text = f"{item.text} {line.strip()}".strip()
                item.add_links(len(BARE_URL.findall(line)))
            continue
        indent = len(m.group(1))
        while len(stack) > 1 and indent < stack[-1][0]:
            stack.pop()
        if indent > stack[-1][0] and stack[-1][1].children:
            nested = Block(BlockType.LIST)
            stack[-1][1].children[-1].children.append(nested)
            stack.append((indent, nested))
        text = line[m.end() :].strip()
        item = Block(BlockType.LIST_ITEM, text=text).add_links(len(BARE_URL.findall(text)))
        stack[-1][1].children.append(item)
    return root


def classify_chunk(lines: list[str], *, allow_headings: bool = True) -> list[Block]:
    """Classify one blank-line separated chunk into blocks."""
    # Setext heading: a text line underlined by === or --- (may lead a chunk).
    if allow_headings and len(lines) >= 2 and _SETEXT.match(lines[1]) and lines[0].strip():
        setext_level = 1 if lines[1].strip().startswith("=") else 2
        rest = classify_chunk(lines[2:], allow_headings=allow_headings) if lines[2:] else []
        title = lines[0].strip()
        return [heading(setext_level, title).add_links(len(BARE_URL.findall(title))), *rest]

    first = lines[0]
    if all(_QUOTE.match(line) for line in lines):
        inner = "\n".join(_QUOTE.sub("", line, count=1) for line in lines)
        return [Block(BlockType.BLOCKQUOTE, children=parse_text(inner))]
    if _FOOTNOTE_DEF.match(first):
        return [_paragraph([_FOOTNOTE_DEF.sub("", first, count=1), *lines[1:]], BlockType.FOOTNOTE)]
    if LIST_MARKER.match(first) and not (
        allow_headings and len(lines) == 1 and _heading_level(first) is not None
    ):
        return [_list(lines)]
    if _is_table(lines):
        links = sum(len(BARE_URL.findall(line)) for line in lines)
        return [Block(BlockType.TABLE).add_links(links)]
    if FIGURE_CAPTION.match(first.strip()):
        return [Block(BlockType.FIGURE)]
    if allow_headings and len(lines) == 1:
        level = _heading_level(first)
        if level is not None:
            return [heading(level, first.strip()).add_links(len(BARE_URL.findall(first)))]
    return [_paragraph(lines)]


def parse_text(text: str) -> list[Block]:
    blocks: list[Block] = []
    for chunk in split_chunks(text):
        blocks.extend(classify_chunk(chunk))
    return blocks
