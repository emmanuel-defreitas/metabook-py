"""
FastMCP server.

Exposes three tools that wrap the exact same service layer used by the REST API:

  search_book_structure  — find a book and return its structural metadata
  upload_book_epub       — upload an EPUB (base64 or URL) and analyse it
  list_supported_schemas — enumerate the schema types the detector understands

Mounting
--------
In main.py the MCP server is mounted on the FastAPI app at /mcp so it can be
reached via HTTP (SSE transport) alongside the REST endpoints:

    app.mount("/mcp", mcp.http_app(path="/"))

If you prefer to run it on stdio (e.g. for local Claude Desktop integration),
call:

    python -c "import asyncio; from metabook_py.mcp_server import mcp; mcp.run()"
"""

import base64
import binascii

import httpx
from fastmcp import FastMCP

from metabook_py.core.config import settings
from metabook_py.core.exceptions import (
    AmbiguousBookError,
    BlobUploadError,
    BookNotFoundError,
    GutendexUnavailableError,
    InvalidEpubError,
    TextUnavailableError,
)
from metabook_py.models.book import AuthorInfo, UploadedBookInfo
from metabook_py.services.blob import upload_epub
from metabook_py.services.counter import DETAIL_LEVELS, build_structure_tree
from metabook_py.services.detector import SCHEMA_DEFINITIONS, detect_schema
from metabook_py.services.discovery import GutendexClient
from metabook_py.services.epub import parse_epub
from metabook_py.services.fetcher import fetch_book_text
from metabook_py.services.store import get_upload_store, scan_update_doc, upload_doc

mcp = FastMCP(
    name="Book Structure MCP",
    instructions=(
        "Use 'search_book_structure' to analyse the structural metadata of any "
        "Project Gutenberg book by title, ISBN, or Gutenberg ID. "
        "Use 'upload_book_epub' to store and analyse your own EPUB file. "
        "Use 'list_supported_schemas' to see which structural types are recognised."
    ),
)


@mcp.tool()
async def search_book_structure(
    title: str | None = None,
    isbn: str | None = None,
    gutenberg_id: int | None = None,
    language: str = "en",
    include_paragraphs: bool = True,
    detail: str = "paragraph",
) -> dict:
    """
    Find a book on Project Gutenberg and return its full structural metadata.

    The response contains counts of chapters, paragraphs, sentences, and words
    per structural node. No actual book text is returned.

    Parameters
    ----------
    title              Fuzzy title search (e.g. "Pride and Prejudice")
    isbn               ISBN-10 or ISBN-13
    gutenberg_id       Direct Gutenberg ID — most precise, use this when
                       a previous call returned a disambiguation list
    language           ISO 639-1 code (default "en")
    include_paragraphs Include per-paragraph node detail (default True;
                       set False for a summary-only response on large books)
    detail             Leaf nesting depth: "paragraph" (default), "sentence",
                       "clause", or "word" — deeper levels nest sentence,
                       clause, and word nodes (counts only, never text)
    """
    if not any([title, isbn, gutenberg_id]):
        return {"error": "Provide at least one of: title, isbn, gutenberg_id."}
    if detail not in DETAIL_LEVELS:
        return {"error": "invalid_detail", "allowed": list(DETAIL_LEVELS)}

    client = GutendexClient()

    try:
        book_info, download_url, is_html = await client.search(
            title=title, isbn=isbn, gutenberg_id=gutenberg_id, language=language
        )
    except BookNotFoundError:
        return {
            "error": "book_not_found",
            "query": {"title": title, "isbn": isbn, "gutenberg_id": gutenberg_id},
        }
    except GutendexUnavailableError as exc:
        return {
            "error": "gutendex_unreachable",
            "detail": exc.reason,
            "hint": (
                "The Gutendex API timed out. Try again shortly."
                if exc.timed_out
                else "The Gutendex API could not be reached. Try again shortly."
            ),
        }
    except AmbiguousBookError as exc:
        return {
            "status": 300,
            "message": (
                "Multiple books matched your query. "
                "Call this tool again with one of the gutenberg_id values below."
            ),
            "matches": [m.model_dump() for m in exc.matches],
        }

    # A book was selected — persist it (unscanned) before fetching its text.
    store = get_upload_store()
    await store.record_gutenberg_book(book_info)

    try:
        text, was_cached = await fetch_book_text(
            book_info.gutenberg_id, download_url, is_html=is_html
        )
    except TextUnavailableError:
        return {
            "error": "text_unavailable",
            "gutenberg_id": book_info.gutenberg_id,
            "hint": "The book exists on Gutenberg but its plain-text file could not be retrieved.",
        }

    schema = detect_schema(text)
    nodes, summary = build_structure_tree(
        text, schema, include_paragraphs=include_paragraphs, detail=detail
    )

    await store.record_scan(book_info.gutenberg_id, scan_update_doc(schema, detail, summary, None))

    return {
        "book": book_info.model_dump(),
        "structure": {
            "schema": schema.name.value,
            "schema_confidence": schema.confidence,
            "summary": summary.model_dump(),
            "nodes": [n.model_dump() for n in nodes],
        },
        "cached": was_cached,
    }


@mcp.tool()
async def upload_book_epub(
    filename: str = "book.epub",
    epub_base64: str | None = None,
    epub_url: str | None = None,
    include_paragraphs: bool = True,
    detail: str = "paragraph",
) -> dict:
    """
    Upload an EPUB, store it in Vercel Blob storage (books/ folder), and
    return its structural metadata — the same analysis as search_book_structure
    but for a caller-supplied file.

    Provide the EPUB one of two ways (exactly one is required):

    epub_base64        the EPUB file's bytes, base64-encoded
    epub_url           a URL to download the EPUB from

    Parameters
    ----------
    filename           name to store the file under (default "book.epub")
    include_paragraphs Include per-paragraph node detail (default True;
                       set False for a summary-only response on large books)

    Metadata (title, authors, language, subjects, ISBN) is extracted from the
    EPUB's package document. No actual book text is returned.
    """
    if (epub_base64 is None) == (epub_url is None):
        return {"error": "Provide exactly one of: epub_base64, epub_url."}
    if detail not in DETAIL_LEVELS:
        return {"error": "invalid_detail", "allowed": list(DETAIL_LEVELS)}

    if epub_base64 is not None:
        try:
            content = base64.b64decode(epub_base64, validate=True)
        except (binascii.Error, ValueError):
            return {"error": "invalid_base64", "hint": "epub_base64 is not valid base64."}
    else:
        try:
            async with httpx.AsyncClient(timeout=60.0, follow_redirects=True) as client:
                resp = await client.get(epub_url)  # type: ignore[arg-type]
                resp.raise_for_status()
                content = resp.content
        except httpx.HTTPError as exc:
            return {"error": "download_failed", "url": epub_url, "detail": str(exc)}

    if len(content) > settings.max_upload_bytes:
        return {"error": "file_too_large", "max_bytes": settings.max_upload_bytes}

    try:
        parsed = parse_epub(content)
    except InvalidEpubError as exc:
        return {"error": "invalid_epub", "detail": exc.reason}

    try:
        blob = await upload_epub(filename, content)
    except BlobUploadError as exc:
        return {"error": "blob_upload_failed", "detail": exc.reason}

    schema = detect_schema(parsed.text)
    nodes, summary = build_structure_tree(
        parsed.text, schema, include_paragraphs=include_paragraphs, detail=detail
    )

    uploaded_book = UploadedBookInfo(
        title=parsed.metadata.title,
        authors=[AuthorInfo(name=a) for a in parsed.metadata.authors],
        language=parsed.metadata.language,
        subjects=parsed.metadata.subjects,
        isbn=parsed.metadata.isbn,
    )
    await get_upload_store().record_upload(
        upload_doc(uploaded_book, blob, schema, detail, summary, None)
    )

    return {
        "book": {
            "source": "upload",
            "title": parsed.metadata.title,
            "authors": parsed.metadata.authors,
            "language": parsed.metadata.language,
            "subjects": parsed.metadata.subjects,
            "isbn": parsed.metadata.isbn,
        },
        "blob": {"url": blob.url, "pathname": blob.pathname, "size_bytes": blob.size},
        "structure": {
            "schema": schema.name.value,
            "schema_confidence": schema.confidence,
            "summary": summary.model_dump(),
            "nodes": [n.model_dump() for n in nodes],
        },
        "spine_document_count": parsed.spine_document_count,
    }


@mcp.tool()
async def list_supported_schemas() -> list[dict]:
    """
    Return all structural schemas the API can detect.

    Each entry includes:
    - name        unique identifier used in responses
    - description human-readable explanation
    - hierarchy   ordered list of structural levels (top → leaf)

    Call this to understand which schema names may appear in
    search_book_structure responses.
    """
    return list(SCHEMA_DEFINITIONS.values())
