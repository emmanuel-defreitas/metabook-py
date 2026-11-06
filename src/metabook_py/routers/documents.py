"""
REST router — /api/documents/

POST /api/documents/analyze      multipart file  → DocumentAnalysisResponse
POST /api/documents/analyze-url  {"url": "…"}    → DocumentAnalysisResponse

Both call services/documents/service.py, as does the MCP tool analyze_document.
"""

from typing import Annotated, Any

from fastapi import APIRouter, File, HTTPException, UploadFile
from starlette.concurrency import run_in_threadpool

from metabook_py.core.config import settings
from metabook_py.core.exceptions import (
    DocumentFetchError,
    DocumentTooLargeError,
    InvalidDocumentError,
    UnsupportedDocumentTypeError,
    UrlNotAllowedError,
)
from metabook_py.core.identity import browser_user_id
from metabook_py.models.document import AnalyzeUrlRequest, DocumentAnalysisResponse
from metabook_py.services.documents.service import analyze_document, analyze_document_url
from metabook_py.services.store import get_upload_store
from metabook_py.services.user_library import UserLibraryStore

router = APIRouter(prefix="/documents", tags=["documents"])

_ERRORS: dict[int | str, dict[str, Any]] = {
    400: {"description": "URL not allowed (scheme, credentials, or non-public address)"},
    413: {"description": "Document exceeds 20 MB"},
    415: {"description": "Content type not accepted"},
    422: {"description": "Document could not be parsed"},
}


def to_http_error(exc: Exception) -> HTTPException:
    """Map document-service errors onto HTTP semantics."""
    match exc:
        case DocumentTooLargeError():
            return HTTPException(413, {"error": "file_too_large", "max_bytes": exc.max_bytes})
        case UnsupportedDocumentTypeError():
            return HTTPException(
                415,
                {
                    "error": "unsupported_type",
                    "content_type": exc.content_type,
                    "allowed": exc.allowed,
                },
            )
        case InvalidDocumentError():
            return HTTPException(422, {"error": "invalid_document", "message": exc.reason})
        case UrlNotAllowedError():
            return HTTPException(400, {"error": "url_not_allowed", "message": exc.reason})
        case DocumentFetchError():
            status = 504 if exc.timed_out else 502
            code = "fetch_timeout" if exc.timed_out else "fetch_failed"
            return HTTPException(status, {"error": code, "message": exc.reason})
    raise exc


_DOC_ERRORS = (
    DocumentTooLargeError,
    UnsupportedDocumentTypeError,
    InvalidDocumentError,
    UrlNotAllowedError,
    DocumentFetchError,
)


async def _save_to_library(result: DocumentAnalysisResponse) -> None:
    """Save the result to the caller's browser library, when there is one."""
    if not browser_user_id.get():
        return
    store = get_upload_store()  # raises when MongoDB is not configured
    if isinstance(store, UserLibraryStore):
        result.record_id = await store.record_document(result)


@router.post(
    "/analyze",
    response_model=DocumentAnalysisResponse,
    responses={k: v for k, v in _ERRORS.items() if k != 400},
    summary="Analyse an uploaded document's semantic structure",
)
async def analyze_upload(
    file: Annotated[UploadFile, File(description="HTML, Markdown, PDF or plain-text file")],
    detail: bool = False,
) -> DocumentAnalysisResponse:
    """
    Parse one document (HTML, Markdown, PDF or plain text) into a section tree
    nested by heading level, with counts per section and in total.

    **No text content is included in the response.**
    """
    # Read one byte past the limit so oversize files are rejected without
    # buffering arbitrarily large bodies.
    data = await file.read(settings.document_max_bytes + 1)
    try:
        result = await run_in_threadpool(
            analyze_document, data, content_type=file.content_type, filename=file.filename, detail=detail
        )
        await _save_to_library(result)
        return result
    except _DOC_ERRORS as exc:
        raise to_http_error(exc) from exc


@router.post(
    "/analyze-url",
    response_model=DocumentAnalysisResponse,
    responses={
        **_ERRORS,
        502: {"description": "Upstream fetch failed"},
        504: {"description": "Upstream fetch timed out (10 s)"},
    },
    summary="Fetch a document by URL and analyse its semantic structure",
)
async def analyze_url(body: AnalyzeUrlRequest, detail: bool = False) -> DocumentAnalysisResponse:
    """
    Fetch a public http(s) URL (10 s limit, 20 MB, private addresses refused)
    and return the same structure as `POST /analyze`.

    **No text content is included in the response.**
    """
    try:
        result = await analyze_document_url(str(body.url), detail=detail)
        await _save_to_library(result)
        return result
    except _DOC_ERRORS as exc:
        raise to_http_error(exc) from exc
