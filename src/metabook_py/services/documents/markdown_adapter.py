"""
Markdown adapter — a block-level CommonMark/GFM subset, no dependencies.

Handles: YAML front matter (skipped), fenced and indented code, ATX and setext
headings, thematic breaks, blockquotes (recursive), nested bullet/ordered
lists, GFM pipe tables, footnote definitions, link reference definitions
(skipped), standalone images (figures) and paragraphs. Inline links are
counted: [text](url), [text][ref], <scheme://…> autolinks and bare URLs.
"""

from __future__ import annotations

import re

from metabook_py.services.documents.blocks import BARE_URL, Block, BlockType, heading

_FENCE = re.compile(r"^\s{0,3}(`{3,}|~{3,})")
_ATX = re.compile(r"^\s{0,3}(#{1,6})(?:[ \t]+(.*?))?(?:[ \t]+#+)?[ \t]*$")
_SETEXT = re.compile(r"^\s{0,3}(=+|-+)[ \t]*$")
_HR = re.compile(r"^\s{0,3}([-*_])(?:[ \t]*\1){2,}[ \t]*$")
_QUOTE = re.compile(r"^\s{0,3}> ?")
_FOOTNOTE_DEF = re.compile(r"^\s{0,3}\[\^[^\]]+\]:[ \t]*(.*)$")
_LINK_DEF = re.compile(r"^\s{0,3}\[[^\]^][^\]]*\]:\s*\S+")
_TABLE_SEP = re.compile(r"^\s*\|?\s*:?-+:?\s*(?:\|\s*:?-+:?\s*)*\|?\s*$")
_LIST = re.compile(r"^(\s*)([-*+]|\d{1,9}[.)])(?:[ \t]+(.*))?$")
_IMAGE_LINE = re.compile(r"^\s*!\[[^\]]*\]\([^)]*\)\s*$")
_INDENTED = re.compile(r"^(?: {4}|\t)")

# Inline syntax
_IMAGE = re.compile(r"!\[([^\]]*)\]\([^)]*\)")
_INLINE_LINK = re.compile(r"(?<!!)\[([^\]]*)\]\([^)]*\)")
_REF_LINK = re.compile(r"(?<!!)\[([^\]]+)\]\[[^\]]*\]")
_AUTOLINK = re.compile(r"<[a-zA-Z][a-zA-Z0-9+.-]{1,31}:[^\s<>]*>")
_FOOTNOTE_REF = re.compile(r"\[\^[^\]]+\]")


def count_links(raw: str) -> int:
    n = 0
    for pattern in (_INLINE_LINK, _REF_LINK, _AUTOLINK):
        n += len(pattern.findall(raw))
        raw = pattern.sub(" ", raw)
    return n + len(BARE_URL.findall(_IMAGE.sub(" ", raw)))


def strip_inline(raw: str) -> str:
    """Visible text: link text kept, URLs/images/footnote markers dropped."""
    text = _IMAGE.sub(" ", raw)
    text = _INLINE_LINK.sub(r"\1", text)
    text = _REF_LINK.sub(r"\1", text)
    text = _AUTOLINK.sub(" ", text)
    text = _FOOTNOTE_REF.sub("", text)
    return re.sub(r"\s+", " ", text).strip()


def _prose(kind: BlockType, raw: str) -> Block:
    return Block(kind, text=strip_inline(raw)).add_links(count_links(raw))


def _heading(level: int, raw: str) -> Block:
    return heading(level, strip_inline(raw)).add_links(count_links(raw))


def _starts_block(line: str) -> bool:
    return bool(
        _FENCE.match(line)
        or _ATX.match(line)
        or _QUOTE.match(line)
        or _HR.match(line)
        or _LIST.match(line)
    )


def _unquote(line: str) -> str:
    """Strip every leading blockquote marker."""
    while _QUOTE.match(line):
        line = _QUOTE.sub("", line, count=1)
    return line


def _is_paragraph_text(line: str) -> bool:
    """Whether a quoted line (outer marker already stripped) can be lazily continued."""
    line = _unquote(line)
    return bool(
        line.strip()
        and not (_ATX.match(line) or _FENCE.match(line) or _HR.match(line) or _SETEXT.match(line))
    )


def _quote_lines(lines: list[str], start: int) -> tuple[list[str], int]:
    """The blockquote at *start*, markers stripped, and the index just past it.

    An unquoted line stays in the quote only as lazy paragraph continuation:
    the previous line must be open paragraph text (not a heading, rule or code)
    and the line itself must not start a new block.
    """
    inner: list[str] = []
    in_fence = False
    i, n = start, len(lines)
    while i < n and lines[i].strip():
        line = lines[i]
        if _QUOTE.match(line):
            content = _QUOTE.sub("", line, count=1)
            if _FENCE.match(_unquote(content)):
                in_fence = not in_fence
            inner.append(content)
        elif not in_fence and _is_paragraph_text(inner[-1]) and not _starts_block(line):
            inner.append(line)
        else:
            break
        i += 1
    return inner, i


def _strip_front_matter(lines: list[str]) -> list[str]:
    if lines and lines[0].strip() == "---":
        for i in range(1, len(lines)):
            if lines[i].strip() in ("---", "..."):
                return lines[i + 1 :]
    return lines


def _list_item_lines(lines: list[str], start: int) -> int:
    """Index just past the list that begins at *start*."""
    j = start + 1
    n = len(lines)
    while j < n:
        line = lines[j]
        if not line.strip():
            nxt = lines[j + 1] if j + 1 < n else ""
            if nxt.strip() and (_LIST.match(nxt) or nxt.startswith((" ", "\t"))):
                j += 1
                continue
            break
        if _LIST.match(line) or line.startswith((" ", "\t")):
            j += 1
            continue
        # Lazy continuation: plain text right after a non-blank line.
        if lines[j - 1].strip() and not _starts_block(line):
            j += 1
            continue
        break
    return j


def _parse_list(lines: list[str]) -> Block:
    first = _LIST.match(lines[0])
    assert first is not None
    base = len(first.group(1))
    items: list[tuple[int, list[str]]] = []  # (content indent, lines)
    for line in lines:
        m = _LIST.match(line)
        if m and len(m.group(1)) <= base + 1:
            content_indent = len(m.group(1)) + len(m.group(2)) + 1
            items.append((content_indent, [m.group(3) or ""]))
        elif items:
            indent, body = items[-1]
            leading = len(line) - len(line.lstrip(" "))
            body.append(line[min(indent, leading) :])
    block = Block(BlockType.LIST)
    for _, body in items:
        children = parse_lines(body)
        item = Block(BlockType.LIST_ITEM)
        if children and children[0].type is BlockType.PARAGRAPH:
            lead = children.pop(0)
            item.text = lead.text
            item.children.extend(lead.children)  # its links
        item.children.extend(children)
        block.children.append(item)
    return block


def parse_lines(lines: list[str]) -> list[Block]:
    blocks: list[Block] = []
    para: list[str] = []

    def flush() -> None:
        if para:
            blocks.append(_prose(BlockType.PARAGRAPH, " ".join(para)))
            para.clear()

    i, n = 0, len(lines)
    while i < n:
        line = lines[i]
        if not line.strip():
            flush()
            i += 1
            continue

        if fence := _FENCE.match(line):
            flush()
            marker = fence.group(1)
            close = re.compile(rf"^\s{{0,3}}{re.escape(marker[0])}{{{len(marker)},}}\s*$")
            i += 1
            while i < n and not close.match(lines[i]):
                i += 1
            blocks.append(Block(BlockType.CODE_BLOCK))
            i += 1
            continue

        if atx := _ATX.match(line):
            flush()
            blocks.append(_heading(len(atx.group(1)), atx.group(2) or ""))
            i += 1
            continue

        if para and (setext := _SETEXT.match(line)):
            level = 1 if setext.group(1).startswith("=") else 2
            blocks.append(_heading(level, " ".join(para)))
            para.clear()
            i += 1
            continue

        if _HR.match(line):
            flush()
            i += 1
            continue

        if _QUOTE.match(line):
            flush()
            inner, i = _quote_lines(lines, i)
            blocks.append(Block(BlockType.BLOCKQUOTE, children=parse_lines(inner)))
            continue

        if fn := _FOOTNOTE_DEF.match(line):
            flush()
            raw = [fn.group(1)]
            i += 1
            while i < n and lines[i].strip() and _INDENTED.match(lines[i]):
                raw.append(lines[i].strip())
                i += 1
            blocks.append(_prose(BlockType.FOOTNOTE, " ".join(raw)))
            continue

        if not para and _LINK_DEF.match(line):
            i += 1
            continue

        if "|" in line and i + 1 < n and _TABLE_SEP.match(lines[i + 1]) and "-" in lines[i + 1]:
            flush()
            links = 0
            while i < n and lines[i].strip() and "|" in lines[i]:
                links += count_links(lines[i])
                i += 1
            blocks.append(Block(BlockType.TABLE).add_links(links))
            continue

        # Indented code wins over list markers at the start of a block; after
        # paragraph text, an indented marker still opens a (nested) list.
        if not para and _INDENTED.match(line):
            while i < n and (_INDENTED.match(lines[i]) or not lines[i].strip()):
                i += 1
            blocks.append(Block(BlockType.CODE_BLOCK))
            continue

        if _LIST.match(line) and _LIST.match(line).group(3):  # type: ignore[union-attr]
            flush()
            end = _list_item_lines(lines, i)
            blocks.append(_parse_list(lines[i:end]))
            i = end
            continue

        if not para and _IMAGE_LINE.match(line):
            blocks.append(Block(BlockType.FIGURE))
            i += 1
            continue

        para.append(line.strip())
        i += 1

    flush()
    return blocks


def parse_markdown(text: str) -> list[Block]:
    lines = text.replace("\r\n", "\n").replace("\r", "\n").split("\n")
    return parse_lines(_strip_front_matter(lines))
