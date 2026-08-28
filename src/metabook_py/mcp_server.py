"""
FastMCP server.

Exposes two tools that wrap the exact same service layer used by the REST API:

  search_book_structure  — find a book and return its structural metadata
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

from fastmcp import FastMCP

from metabook_py.core.exceptions import AmbiguousBookError, BookNotFoundError, TextUnavailableError
from metabook_py.services.counter import build_structure_tree
from metabook_py.services.detector import SCHEMA_DEFINITIONS, detect_schema
from metabook_py.services.discovery import GutendexClient
from metabook_py.services.fetcher import fetch_book_text

mcp = FastMCP(
    name="Book Structure MCP",
    instructions=(
        "Use 'search_book_structure' to analyse the structural metadata of any "
        "Project Gutenberg book by title, ISBN, or Gutenberg ID. "
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
    """
    if not any([title, isbn, gutenberg_id]):
        return {"error": "Provide at least one of: title, isbn, gutenberg_id."}

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
    except AmbiguousBookError as exc:
        return {
            "status": 300,
            "message": (
                "Multiple books matched your query. "
                "Call this tool again with one of the gutenberg_id values below."
            ),
            "matches": [m.model_dump() for m in exc.matches],
        }

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
    nodes, summary = build_structure_tree(text, schema, include_paragraphs=include_paragraphs)

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
