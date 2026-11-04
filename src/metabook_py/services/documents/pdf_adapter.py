"""
PDF adapter (pypdf).

PDFs carry no semantic tags, so structure is inferred from layout:
- Lines are grouped from positioned text runs.
- Body size = the font size covering the most characters. Short lines set at
  least 15% larger are headings; distinct heading sizes are ranked largest = 1.
- Paragraphs break on a vertical gap well above the usual line spacing.
- Non-heading groups go through the plain-text chunk classifier (lists,
  figure captions, footnote definitions). Tables and code are not detected.
"""

from __future__ import annotations

import math
import re
import statistics
from collections import Counter
from dataclasses import dataclass
from io import BytesIO

from metabook_py.core.exceptions import InvalidDocumentError
from metabook_py.services.documents.blocks import BARE_URL, Block, heading
from metabook_py.services.documents.text_adapter import classify_chunk

_HEADING_RATIO = 1.15
_PAGE_NUMBER = re.compile(r"^\s*(?:page\s+)?\d{1,4}(?:\s*(?:/|of)\s*\d{1,4})?\s*$", re.IGNORECASE)
_SENTENCE_END = re.compile(r"[.!?:\"”)]$")


@dataclass
class _Line:
    page: int
    y: float
    size: float
    text: str


def _extract_lines(data: bytes) -> tuple[list[_Line], int]:
    from pypdf import PdfReader
    from pypdf.errors import PyPdfError

    try:
        reader = PdfReader(BytesIO(data))
        if reader.is_encrypted and not reader.decrypt(""):
            raise InvalidDocumentError("encrypted PDF")
        pages = list(reader.pages)
    except InvalidDocumentError:
        raise
    except (PyPdfError, ValueError, KeyError, TypeError, OSError) as exc:
        raise InvalidDocumentError(f"unreadable PDF: {exc}") from exc

    lines: list[_Line] = []
    for page_no, page in enumerate(pages):
        current: list[tuple[float, float, str]] = []  # (y, size, text)

        def visit(text, cm, tm, _font_dict, font_size, _current=current):
            if not text:
                return
            scale = math.hypot(tm[2], tm[3]) * math.hypot(cm[2], cm[3]) or 1.0
            y = tm[4] * cm[1] + tm[5] * cm[3] + cm[5]
            _current.append((y, float(font_size or 0) * scale, text))

        try:
            page.extract_text(visitor_text=visit)
        except Exception as exc:  # pypdf raises many types on broken streams
            raise InvalidDocumentError(f"unreadable PDF page {page_no + 1}: {exc}") from exc

        lines.extend(_group_runs(page_no, current))
    return [ln for ln in lines if not _PAGE_NUMBER.match(ln.text)], len(pages)


def _group_runs(page_no: int, runs: list[tuple[float, float, str]]) -> list[_Line]:
    """Group positioned text runs into lines by baseline (runs may carry "\n")."""
    out: list[_Line] = []
    line_y: float | None = None
    sizes: list[float] = []
    parts: list[str] = []
    for y, size, text in runs:
        for k, piece in enumerate(text.split("\n")):
            if parts and (k > 0 or (line_y is not None and abs(y - line_y) > 1.0)):
                joined = "".join(parts).strip()
                if joined and line_y is not None:
                    out.append(_Line(page_no, line_y, max(sizes), joined))
                parts, sizes = [], []
            if piece.strip():
                if not parts:
                    line_y = y
                parts.append(piece)
                sizes.append(size)
    joined = "".join(parts).strip()
    if joined and line_y is not None:
        out.append(_Line(page_no, line_y, max(sizes), joined))
    return out


def _round(size: float) -> float:
    return round(size * 2) / 2


def _dehyphenate(lines: list[str]) -> list[str]:
    out: list[str] = []
    for line in lines:
        if out and re.search(r"[A-Za-z]-$", out[-1]) and line[:1].islower():
            out[-1] = out[-1][:-1] + line
        else:
            out.append(line)
    return out


def parse_pdf(data: bytes) -> tuple[list[Block], int]:
    lines, page_count = _extract_lines(data)
    if not lines:
        return [], page_count

    chars: Counter[float] = Counter()
    for ln in lines:
        chars[_round(ln.size)] += len(ln.text)
    body = chars.most_common(1)[0][0]

    def is_heading(ln: _Line) -> bool:
        return (
            _round(ln.size) >= body * _HEADING_RATIO
            and len(ln.text.split()) <= 15
            and not re.search(r"[.,;]$", ln.text)
        )

    heading_sizes = sorted({_round(ln.size) for ln in lines if is_heading(ln)}, reverse=True)
    level_of = {size: min(6, i + 1) for i, size in enumerate(heading_sizes)}

    gaps = [
        a.y - b.y
        for a, b in zip(lines, lines[1:], strict=False)
        if a.page == b.page and not is_heading(a) and not is_heading(b) and 0 < a.y - b.y
    ]
    spacing = statistics.median(gaps) if gaps else body * 1.2

    blocks: list[Block] = []
    group: list[str] = []
    prev: _Line | None = None

    def flush() -> None:
        if group:
            blocks.extend(classify_chunk(_dehyphenate(group), allow_headings=False))
            group.clear()

    for ln in lines:
        if is_heading(ln):
            # Consecutive same-size heading lines are one wrapped heading.
            if (
                prev is not None
                and is_heading(prev)
                and _round(prev.size) == _round(ln.size)
                and prev.page == ln.page
                and 0 < prev.y - ln.y <= ln.size * 1.6
                and blocks
            ):
                blocks[-1].text = f"{blocks[-1].text} {ln.text}"
                blocks[-1].add_links(len(BARE_URL.findall(ln.text)))
            else:
                flush()
                level = level_of[_round(ln.size)]
                blocks.append(heading(level, ln.text).add_links(len(BARE_URL.findall(ln.text))))
        else:
            new_para = prev is None or is_heading(prev)
            if prev is not None and not new_para:
                if prev.page != ln.page:
                    new_para = bool(_SENTENCE_END.search(prev.text))
                else:
                    new_para = (prev.y - ln.y) > spacing * 1.4 or prev.y < ln.y
            if new_para:
                flush()
            group.append(ln.text)
        prev = ln
    flush()
    return blocks, page_count
