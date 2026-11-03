"""
HTML adapter (stdlib html.parser, tolerant of malformed markup).

Semantic containers (lists, items, tables, figures, blockquotes, code,
footnotes, references) become blocks; generic containers (div, section, …)
only act as boundaries. Loose text becomes paragraphs. <a href> is a link.
"""

from __future__ import annotations

import re
from html.parser import HTMLParser

from metabook_py.services.documents.blocks import Block, BlockType, heading

_SKIP = frozenset({"script", "style", "template", "noscript", "head", "svg", "math", "iframe"})
_LEAF = frozenset({"p", "h1", "h2", "h3", "h4", "h5", "h6", "dt", "dd", "summary", "address"})
_BOUNDARY = frozenset(
    {
        "div", "section", "article", "main", "aside", "header", "footer", "nav", "body",
        "html", "form", "fieldset", "details", "dl", "hr", "center", "figcaption",
    }
)  # fmt: skip
_VOID = frozenset({"img", "br", "hr", "input", "meta", "link", "source", "wbr", "area", "col"})
_OPAQUE = (BlockType.TABLE, BlockType.FIGURE, BlockType.CODE_BLOCK)  # inner text not counted
_FOOTNOTE_ID = re.compile(r"^(?:fn|footnote|endnote)[-_:]?\w", re.IGNORECASE)


class _Builder(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.root: list[Block] = []
        # (tag, block or None) — None marks a non-semantic element.
        self.stack: list[tuple[str, Block | None]] = []
        self.skip_depth = 0
        self.leaf: str | None = None  # open leaf tag (p, h1, …)
        self.buf: list[str] = []
        self.buf_links = 0

    # ── helpers ────────────────────────────────────────────────────────────────

    def _container(self) -> Block | None:
        for _, block in reversed(self.stack):
            if block is not None:
                return block
        return None

    def _append(self, block: Block) -> None:
        parent = self._container()
        (parent.children if parent else self.root).append(block)

    def _opaque(self) -> Block | None:
        parent = self._container()
        return parent if parent is not None and parent.type in _OPAQUE else None

    def _flush(self) -> None:
        text = re.sub(r"\s+", " ", "".join(self.buf)).strip()
        links = self.buf_links
        leaf = self.leaf
        self.buf, self.buf_links = [], 0
        if not text and not links:
            return
        if leaf and leaf[0] == "h" and leaf[1:].isdigit():
            self._append(heading(int(leaf[1]), text).add_links(links))
            return
        parent = self._container()
        # Text directly inside an item / footnote / reference is that block's own text.
        if parent is not None and parent.type in (
            BlockType.LIST_ITEM,
            BlockType.FOOTNOTE,
            BlockType.REFERENCE,
        ):
            parent.text = f"{parent.text} {text}".strip()
            parent.add_links(links)
            return
        self._append(Block(BlockType.PARAGRAPH, text=text).add_links(links))

    def _semantic_block(self, tag: str, attrs: dict[str, str]) -> Block | None:
        role = attrs.get("role", "")
        classes = set(attrs.get("class", "").lower().split())
        if role in ("doc-footnote", "doc-endnote") or classes & {"footnote", "endnote"}:
            return Block(BlockType.FOOTNOTE)
        if tag in ("li", "div", "aside", "p", "section") and _FOOTNOTE_ID.match(
            attrs.get("id", "")
        ):
            return Block(BlockType.FOOTNOTE)
        if role == "doc-biblioentry":
            return Block(BlockType.REFERENCE)
        if tag in ("ul", "ol"):
            return Block(BlockType.LIST)
        if tag == "li":
            return Block(BlockType.LIST_ITEM)
        if tag == "table":
            return Block(BlockType.TABLE)
        if tag == "figure":
            return Block(BlockType.FIGURE)
        if tag == "pre":
            return Block(BlockType.CODE_BLOCK)
        if tag == "blockquote":
            return Block(BlockType.BLOCKQUOTE)
        return None

    # ── parser callbacks ───────────────────────────────────────────────────────

    def handle_starttag(self, tag: str, attrs_list: list[tuple[str, str | None]]) -> None:
        if self.skip_depth:
            if tag in _SKIP:
                self.skip_depth += 1
            return
        if tag in _SKIP:
            self.skip_depth = 1
            return
        attrs = {k: (v or "") for k, v in attrs_list}

        if tag == "a" and attrs.get("href"):
            if (opaque := self._opaque()) is not None:
                opaque.add_links(1)
            else:
                self.buf_links += 1
            return
        if tag == "img":
            if self._opaque() is None:
                self._append(Block(BlockType.FIGURE))
            return
        if tag == "br":
            self.buf.append(" ")
            return

        block = self._semantic_block(tag, attrs)
        if block is not None or tag in _LEAF or tag in _BOUNDARY:
            if self._opaque() is not None:
                if block is not None and block.type is BlockType.TABLE:
                    self._opaque().children.append(block)  # type: ignore[union-attr]
                    self.stack.append((tag, block))
                return
            self._flush()
            self.leaf = tag if tag in _LEAF and block is None else None
            if block is not None:
                self._append(block)
            if tag not in _VOID:
                self.stack.append((tag, block))

    def handle_startendtag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        self.handle_starttag(tag, attrs)
        if tag not in _VOID:
            self.handle_endtag(tag)

    def handle_endtag(self, tag: str) -> None:
        if self.skip_depth:
            if tag in _SKIP:
                self.skip_depth -= 1
            return
        if tag in _VOID or tag == "a":
            return
        # Inside a table/figure/pre only tags opened within it may close.
        floor = 0
        if self._opaque() is not None:
            floor = max(i for i, (_, b) in enumerate(self.stack) if b is not None)
        if not any(t == tag for t, _ in self.stack[floor:]):
            if tag == self.leaf:
                self._flush()
                self.leaf = None
            return  # stray end tag
        if self._opaque() is None:
            self._flush()
        while self.stack:
            t, _ = self.stack.pop()
            if t == tag:
                break
        if tag == self.leaf:
            self.leaf = None

    def handle_data(self, data: str) -> None:
        if self.skip_depth or self._opaque() is not None:
            return
        self.buf.append(data)

    def close(self) -> None:
        super().close()
        self._flush()


def parse_html(text: str) -> list[Block]:
    builder = _Builder()
    builder.feed(text)
    builder.close()
    return builder.root
