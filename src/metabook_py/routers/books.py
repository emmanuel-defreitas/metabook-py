"""
REST router — /api/books/

GET  /api/books/structure         → BookStructureResponse | DisambiguationResult
GET  /api/books/structure/schemas → list[SchemaInfo]
POST /api/books/upload            → BookUploadResponse
"""

import time
from datetime import UTC, datetime
from typing import Annotated

from fastapi import APIRouter, Depends, File, HTTPException, Query, UploadFile
from fastapi.responses import JSONResponse

from metabook_py.core.config import settings
from metabook_py.core.exceptions import (
    AmbiguousBookError,
    BlobUploadError,
    BookNotFoundError,
    GutendexUnavailableError,
    InvalidEpubError,
    TextUnavailableError,
)
from metabook_py.models.book import (
    AuthorInfo,
    BlobInfo,
    DisambiguationResult,
    SchemaInfo,
    UploadedBookInfo,
)
from metabook_py.models.structure import (
    BookStructureResponse,
    BookUploadResponse,
    MetaInfo,
    StructureDetail,
    UploadMetaInfo,
)
from metabook_py.services.blob import upload_epub
from metabook_py.services.counter import build_structure_tree
from metabook_py.services.detector import SCHEMA_DEFINITIONS, detect_schema
from metabook_py.services.discovery import GutendexClient
from metabook_py.services.epub import parse_epub
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
        502: {"description": "Gutendex API is unreachable"},
        504: {"description": "Gutendex API timed out"},
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
    except GutendexUnavailableError as exc:
        raise HTTPException(
            status_code=504 if exc.timed_out else 502,
            detail={"error": "gutendex_unreachable", "message": exc.reason},
        ) from exc

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


@router.post(
    "/upload",
    response_model=BookUploadResponse,
    status_code=201,
    responses={
        400: {"description": "File is not a valid EPUB"},
        413: {"description": "File exceeds the upload size limit"},
        502: {"description": "Upload to blob storage failed"},
    },
    summary="Upload an EPUB and analyse its structure",
)
async def upload_book(
    file: Annotated[UploadFile, File(description="EPUB file (.epub)")],
    include_paragraphs: bool = Query(
        True, description="Include per-paragraph node detail in the response"
    ),
) -> BookUploadResponse:
    """
    Upload an EPUB, store it in Vercel Blob storage (`books/` folder), then
    walk its package document and spine XHTML files to extract metadata and
    text, detect the structural schema, and return the same metadata tree as
    `GET /structure`.

    **No actual text content is included in the response.**
    """
    filename = file.filename or "book.epub"
    if not filename.lower().endswith(".epub"):
        raise HTTPException(
            status_code=400,
            detail={"error": "invalid_file", "message": "Only .epub files are accepted."},
        )

    content = await file.read()
    if len(content) > settings.max_upload_bytes:
        raise HTTPException(
            status_code=413,
            detail={
                "error": "file_too_large",
                "max_bytes": settings.max_upload_bytes,
            },
        )

    t0 = time.monotonic()

    # ── 1. Parse EPUB (validate BEFORE paying for storage) ─────────────────────
    try:
        parsed = parse_epub(content)
    except InvalidEpubError as exc:
        raise HTTPException(
            status_code=400,
            detail={"error": "invalid_epub", "message": exc.reason},
        ) from exc

    # ── 2. Upload to Vercel Blob (folder: books/) ──────────────────────────────
    try:
        blob = await upload_epub(filename, content)
    except BlobUploadError as exc:
        raise HTTPException(
            status_code=502,
            detail={"error": "blob_upload_failed", "message": exc.reason},
        ) from exc

    # ── 3. Detect schema + build metadata tree ─────────────────────────────────
    schema = detect_schema(parsed.text)
    nodes, summary = build_structure_tree(
        parsed.text, schema, include_paragraphs=include_paragraphs
    )

    processing_ms = int((time.monotonic() - t0) * 1000)

    return BookUploadResponse(
        book=UploadedBookInfo(
            title=parsed.metadata.title,
            authors=[AuthorInfo(name=a) for a in parsed.metadata.authors],
            language=parsed.metadata.language,
            subjects=parsed.metadata.subjects,
            isbn=parsed.metadata.isbn,
        ),
        blob=BlobInfo(url=blob.url, pathname=blob.pathname, size_bytes=blob.size),
        structure=StructureDetail(
            **{"schema": schema.name.value},
            schema_confidence=schema.confidence,
            summary=summary,
            nodes=[n.model_dump() for n in nodes],
        ),
        meta=UploadMetaInfo(
            uploaded_at=datetime.now(UTC),
            spine_document_count=parsed.spine_document_count,
            processing_time_ms=processing_ms,
        ),
    )
