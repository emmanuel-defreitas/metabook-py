"""
REST router — /api/books/

GET  /api/books/structure         → BookStructureResponse | DisambiguationResult
GET  /api/books/structure/schemas → list[SchemaInfo]
POST /api/books/upload            → BookUploadResponse
GET  /api/books/uploads           → list[UploadRecord]
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
    TokenizerNotFoundError,
    TokenizerUnavailableError,
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
    TokenizerInfo,
    UploadMetaInfo,
)
from metabook_py.models.upload import UploadRecord
from metabook_py.services.blob import upload_epub
from metabook_py.services.counter import DETAIL_LEVELS, build_structure_tree
from metabook_py.services.detector import SCHEMA_DEFINITIONS, detect_schema
from metabook_py.services.discovery import GutendexClient
from metabook_py.services.epub import parse_epub
from metabook_py.services.fetcher import fetch_book_text
from metabook_py.services.store import (
    get_upload_store,
    scan_update_doc,
    upload_doc,
)
from metabook_py.services.tokenizers import TokenEncoder, get_encoder

router = APIRouter(prefix="/books", tags=["books"])


# ── Dependency ─────────────────────────────────────────────────────────────────


def get_gutendex_client() -> GutendexClient:
    return GutendexClient()


GutendexDep = Annotated[GutendexClient, Depends(get_gutendex_client)]


def _validate_detail(detail: str) -> None:
    if detail not in DETAIL_LEVELS:
        raise HTTPException(
            status_code=422,
            detail={
                "error": "invalid_detail",
                "allowed": list(DETAIL_LEVELS),
            },
        )


def _resolve_encoder(tokenizer: str | None) -> TokenEncoder | None:
    """Resolve the optional ?tokenizer= name into an encoder, mapping resolver
    errors onto HTTP semantics: a bad name is the client's fault (422), a
    fetch failure on cold start is transient (503)."""
    if tokenizer is None:
        return None
    try:
        return get_encoder(tokenizer)
    except TokenizerNotFoundError as exc:
        raise HTTPException(
            status_code=422,
            detail={
                "error": "tokenizer_not_found",
                "tokenizer": exc.name,
                "message": exc.reason,
            },
        ) from exc
    except TokenizerUnavailableError as exc:
        raise HTTPException(
            status_code=503,
            detail={
                "error": "tokenizer_unavailable",
                "tokenizer": exc.name,
                "message": exc.reason,
            },
        ) from exc


def _tokenizer_info(encoder: TokenEncoder | None) -> TokenizerInfo | None:
    """Echo the resolved tokenizer in the metadata block — a token count is
    never reported without the scheme that produced it."""
    if encoder is None:
        return None
    return TokenizerInfo(name=encoder.name, vocab_size=encoder.vocab_size)


_TOKENIZER_QUERY = Query(
    None,
    description=(
        "Hugging Face tokenizer repository (e.g. bert-base-uncased). When given, "
        "token counts are included alongside word counts at every level of the "
        "structure tree, and the metadata echoes the tokenizer name and vocabulary "
        "size. When omitted, no token counts are computed."
    ),
)


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
        422: {"description": "Text could not be retrieved, or unknown tokenizer"},
        502: {"description": "Gutendex API is unreachable"},
        503: {"description": "Tokenizer could not be fetched (transient failure)"},
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
    detail: str = Query(
        "paragraph",
        description="Leaf nesting depth: paragraph | sentence | clause | word",
    ),
    tokenizer: str | None = _TOKENIZER_QUERY,
) -> BookStructureResponse:
    """
    Locate a book on Project Gutenberg via the Gutendex API, download and
    parse its full text, and return a structural metadata tree — paragraph
    counts, sentence counts, and word counts per node (plus token counts
    when a `tokenizer` is given).

    **No actual text content is included in the response.**

    At least one of `title`, `isbn`, or `gutenberg_id` is required.
    """
    if not any([title, isbn, gutenberg_id]):
        raise HTTPException(
            status_code=422,
            detail="At least one of 'title', 'isbn', or 'gutenberg_id' is required.",
        )
    _validate_detail(detail)
    encoder = _resolve_encoder(tokenizer)

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

    # A book was selected from the search results — persist it (unscanned)
    # so a later text-fetch failure still leaves a record behind.
    store = get_upload_store()
    await store.record_gutenberg_book(book_info)

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
    nodes, summary = build_structure_tree(
        text, schema, include_paragraphs=include_paragraphs, detail=detail, encoder=encoder
    )

    # The scan ran — mark the stored document as scanned with its results.
    await store.record_scan(
        book_info.gutenberg_id, scan_update_doc(schema, detail, summary, encoder)
    )

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
            tokenizer=_tokenizer_info(encoder),
        ),
    )


@router.post(
    "/upload",
    response_model=BookUploadResponse,
    status_code=201,
    responses={
        400: {"description": "File is not a valid EPUB"},
        413: {"description": "File exceeds the upload size limit"},
        422: {"description": "Unknown tokenizer or invalid detail level"},
        502: {"description": "Upload to blob storage failed"},
        503: {"description": "Tokenizer could not be fetched (transient failure)"},
    },
    summary="Upload an EPUB and analyse its structure",
)
async def upload_book(
    file: Annotated[UploadFile, File(description="EPUB file (.epub)")],
    include_paragraphs: bool = Query(
        True, description="Include per-paragraph node detail in the response"
    ),
    detail: str = Query(
        "paragraph",
        description="Leaf nesting depth: paragraph | sentence | clause | word",
    ),
    tokenizer: str | None = _TOKENIZER_QUERY,
) -> BookUploadResponse:
    """
    Upload an EPUB, store it in Vercel Blob storage (`books/` folder), then
    walk its package document and spine XHTML files to extract metadata and
    text, detect the structural schema, and return the same metadata tree as
    `GET /structure`.

    **No actual text content is included in the response.**
    """
    _validate_detail(detail)
    encoder = _resolve_encoder(tokenizer)
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
        parsed.text, schema, include_paragraphs=include_paragraphs, detail=detail, encoder=encoder
    )

    uploaded_book = UploadedBookInfo(
        title=parsed.metadata.title,
        authors=[AuthorInfo(name=a) for a in parsed.metadata.authors],
        language=parsed.metadata.language,
        subjects=parsed.metadata.subjects,
        isbn=parsed.metadata.isbn,
    )
    blob_info = BlobInfo(url=blob.url, pathname=blob.pathname, size_bytes=blob.size)

    # ── 4. Persist the upload document (metadata + scan results, never the tree)
    await get_upload_store().record_upload(
        upload_doc(uploaded_book, blob, schema, detail, summary, encoder)
    )

    processing_ms = int((time.monotonic() - t0) * 1000)

    return BookUploadResponse(
        book=uploaded_book,
        blob=blob_info,
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
            tokenizer=_tokenizer_info(encoder),
        ),
    )


@router.get(
    "/uploads",
    response_model=list[UploadRecord],
    summary="List stored uploads",
)
async def list_uploads(
    limit: int = Query(100, ge=1, le=500, description="Max records to return"),
    source: str | None = Query(
        None, description="Filter by origin: gutenberg | upload", pattern="^(gutenberg|upload)$"
    ),
) -> list[UploadRecord]:
    """
    List the documents persisted in the `uploads` collection — one per book
    uploaded or selected from search results, newest first. Each carries the
    book metadata, the Vercel Blob link (uploads), and the current scan state
    (scanned, scope, schema, total tokens). Requires MONGODB_URI to be set;
    returns an empty list otherwise.
    """
    docs = await get_upload_store().list_uploads(limit=limit, source=source)
    return [UploadRecord.model_validate(doc) for doc in docs]
