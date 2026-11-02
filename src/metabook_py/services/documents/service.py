"""
Single-document analysis service — the one function REST and MCP both call.
"""

from __future__ import annotations

import hashlib
import time
from datetime import UTC, datetime
from pathlib import PurePosixPath

from metabook_py.core.config import settings
from metabook_py.core.exceptions import (
    DocumentTooLargeError,
    InvalidDocumentError,
    UnsupportedDocumentTypeError,
)
from metabook_py.models.document import (
    DocumentAnalysisResponse,
    DocumentFormat,
    DocumentInfo,
    DocumentMeta,
    DocumentStructure,
)
from metabook_py.services.documents.blocks import Block
from metabook_py.services.documents.builder import build_document_structure
from metabook_py.services.native import active_parser, native_core

CONTENT_TYPES: dict[str, DocumentFormat] = {
    "text/html": "html",
    "application/xhtml+xml": "html",
    "text/markdown": "markdown",
    "text/x-markdown": "markdown",
    "application/pdf": "pdf",
    "text/plain": "text",
}
EXTENSIONS: dict[str, DocumentFormat] = {
    ".html": "html",
    ".htm": "html",
    ".xhtml": "html",
    ".md": "markdown",
    ".markdown": "markdown",
    ".pdf": "pdf",
    ".txt": "text",
    ".text": "text",
}
_CANONICAL_TYPE: dict[DocumentFormat, str] = {
    "html": "text/html",
    "markdown": "text/markdown",
    "pdf": "application/pdf",
    "text": "text/plain",
}
_GENERIC_TYPES = {"", "application/octet-stream", "binary/octet-stream"}


def resolve_format(content_type: str | None, name: str | None) -> tuple[DocumentFormat, str]:
    """Return (format, normalized content type). Raises UnsupportedDocumentTypeError."""
    ctype = (content_type or "").split(";", 1)[0].strip().lower()
    if ctype in CONTENT_TYPES:
        return CONTENT_TYPES[ctype], ctype
    if ctype in _GENERIC_TYPES and name:
        fmt = EXTENSIONS.get(PurePosixPath(name.split("?", 1)[0]).suffix.lower())
        if fmt is not None:
            return fmt, _CANONICAL_TYPE[fmt]
    raise UnsupportedDocumentTypeError(content_type, sorted(CONTENT_TYPES))


def _decode(data: bytes) -> str:
    if data.startswith(b"\xef\xbb\xbf"):
        data = data[3:]
    if b"\x00" in data[:4096]:
        raise InvalidDocumentError("binary content in a text document")
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return data.decode("latin-1", errors="replace")


def parse_blocks(data: bytes, fmt: DocumentFormat) -> tuple[list[Block], int | None]:
    """Run the format adapter: bytes → (block list, page count for PDFs)."""
    if fmt == "pdf":
        from metabook_py.services.documents.pdf_adapter import parse_pdf

        if not data.lstrip()[:5].startswith(b"%PDF-"):
            raise InvalidDocumentError("missing %PDF- header")
        return parse_pdf(data)
    text = _decode(data)
    if fmt == "html":
        from metabook_py.services.documents.html_adapter import parse_html

        return parse_html(text), None
    if fmt == "markdown":
        from metabook_py.services.documents.markdown_adapter import parse_markdown

        return parse_markdown(text), None
    from metabook_py.services.documents.text_adapter import parse_text

    return parse_text(text), None


def _block_tuple(block: Block) -> tuple:
    return (str(block.type), block.text, block.level, [_block_tuple(c) for c in block.children])


def build_structure(fmt: DocumentFormat, data: bytes) -> tuple[DocumentStructure, int | None]:
    """Adapter + structure builder, on the active parser backend."""
    core = native_core()
    if core is not None and fmt in ("markdown", "text"):
        return DocumentStructure.model_validate_json(core.analyze_json(fmt, _decode(data))), None
    blocks, page_count = parse_blocks(data, fmt)
    if core is not None:
        payload = core.build_structure_json([_block_tuple(b) for b in blocks])
        return DocumentStructure.model_validate_json(payload), page_count
    sections, totals = build_document_structure(blocks)
    return DocumentStructure(totals=totals, sections=sections), page_count


def analyze_document(
    data: bytes,
    *,
    content_type: str | None,
    filename: str | None = None,
    source_url: str | None = None,
) -> DocumentAnalysisResponse:
    """
    Analyse one document and return its semantic structure — counts only.

    Raises DocumentTooLargeError, UnsupportedDocumentTypeError, InvalidDocumentError.
    """
    if len(data) > settings.document_max_bytes:
        raise DocumentTooLargeError(settings.document_max_bytes)
    fmt, ctype = resolve_format(content_type, filename or source_url)

    t0 = time.monotonic()
    structure, page_count = build_structure(fmt, data)
    processing_ms = int((time.monotonic() - t0) * 1000)

    return DocumentAnalysisResponse(
        document=DocumentInfo(
            format=fmt,
            content_type=ctype,
            size_bytes=len(data),
            sha256=hashlib.sha256(data).hexdigest(),
            page_count=page_count,
            filename=filename,
            source_url=source_url,
        ),
        structure=structure,
        meta=DocumentMeta(
            parser=active_parser(),  # type: ignore[arg-type]
            processing_time_ms=processing_ms,
            analyzed_at=datetime.now(UTC),
        ),
    )


async def analyze_document_url(url: str) -> DocumentAnalysisResponse:
    """Fetch *url* (SSRF-checked, 10 s, 20 MB) and analyse it.

    Raises everything analyze_document raises, plus UrlNotAllowedError and
    DocumentFetchError.
    """
    from metabook_py.services.documents.fetch import fetch_document

    data, content_type, _final_url = await fetch_document(url)
    return analyze_document(data, content_type=content_type, source_url=url)
