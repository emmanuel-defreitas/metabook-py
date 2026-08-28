"""
REST router — /api/books/

GET /api/books/structure         → BookStructureResponse | DisambiguationResult
GET /api/books/structure/schemas → list[SchemaInfo]
"""

import time
from datetime import UTC, datetime
from typing import Annotated

from fastapi import APIRouter, Depends, HTTPException, Query
from fastapi.responses import JSONResponse

from metabook_py.core.exceptions import AmbiguousBookError, BookNotFoundError, TextUnavailableError
from metabook_py.models.book import DisambiguationResult, SchemaInfo
from metabook_py.models.structure import BookStructureResponse, MetaInfo, StructureDetail
from metabook_py.services.counter import build_structure_tree
from metabook_py.services.detector import SCHEMA_DEFINITIONS, detect_schema
from metabook_py.services.discovery import GutendexClient
from metabook_py.services.fetcher import fetch_book_text

router = APIRouter(prefix="/books", tags=["books"])


# ── Dependency ─────────────────────────────────────────────────────────────────


def get_gutendex_client() -> GutendexClient:
    return GutendexClient()


GutendexDep = Annotated[GutendexClient, Depends(get_gutendex_client)]


# ── Endpoints ──────────────────────────────────────────────────────────────────


@router.get(
    "/structure/schemas",
    response_model=list[SchemaInfo],
    summary="List supported structural schemas",
)
async def list_schemas() -> list[SchemaInfo]:
    """Return all schema types the detector can identify."""
    return [SchemaInfo(**v) for v in SCHEMA_DEFINITIONS.values()]


@router.get(
    "/structure",
    response_model=BookStructureResponse,
    responses={
        300: {"description": "Multiple books found — disambiguate with gutenberg_id"},
        404: {"description": "No book matched the query"},
        422: {"description": "Book found but its text could not be retrieved"},
    },
    summary="Analyse book structure",
)
async def get_book_structure(
    client: GutendexDep,
    title: str | None = Query(None, description="Fuzzy title search"),
    isbn: str | None = Query(None, description="ISBN-10 or ISBN-13"),
    gutenberg_id: int | None = Query(None, description="Project Gutenberg book ID"),
    language: str = Query("en", description="ISO 639-1 language code filter"),
    include_paragraphs: bool = Query(
        True, description="Include per-paragraph node detail in the response"
    ),
) -> BookStructureResponse:
    """
    Locate a book on Project Gutenberg via the Gutendex API, download and
    parse its full text, and return a structural metadata tree — paragraph
    counts, sentence counts, and word counts per node.

    **No actual text content is included in the response.**

    At least one of `title`, `isbn`, or `gutenberg_id` is required.
    """
    if not any([title, isbn, gutenberg_id]):
        raise HTTPException(
            status_code=422,
            detail="At least one of 'title', 'isbn', or 'gutenberg_id' is required.",
        )

    t0 = time.monotonic()

    # ── 1. Discover book ───────────────────────────────────────────────────────
    try:
        book_info, download_url, is_html = await client.search(
            title=title, isbn=isbn, gutenberg_id=gutenberg_id, language=language
        )
    except BookNotFoundError as exc:
        raise HTTPException(
            status_code=404,
            detail={
                "error": "book_not_found",
                "query": {"title": title, "isbn": isbn, "gutenberg_id": gutenberg_id},
            },
        ) from exc
    except AmbiguousBookError as exc:
        return JSONResponse(  # type: ignore[return-value]
            status_code=300,
            content=DisambiguationResult(matches=exc.matches).model_dump(),
        )

    # ── 2. Fetch text ──────────────────────────────────────────────────────────
    try:
        text, was_cached = await fetch_book_text(
            book_info.gutenberg_id, download_url, is_html=is_html
        )
    except TextUnavailableError as exc:
        raise HTTPException(
            status_code=422,
            detail={
                "error": "text_unavailable",
                "gutenberg_id": book_info.gutenberg_id,
            },
        ) from exc

    # ── 3. Detect schema ───────────────────────────────────────────────────────
    schema = detect_schema(text)

    # ── 4. Build metadata tree ─────────────────────────────────────────────────
    nodes, summary = build_structure_tree(text, schema, include_paragraphs=include_paragraphs)

    processing_ms = int((time.monotonic() - t0) * 1000)

    return BookStructureResponse(
        book=book_info,
        structure=StructureDetail(
            **{"schema": schema.name.value},
            schema_confidence=schema.confidence,
            summary=summary,
            nodes=[n.model_dump() for n in nodes],
        ),
        meta=MetaInfo(
            fetched_at=datetime.now(UTC),
            cached=was_cached,
            processing_time_ms=processing_ms,
        ),
    )
