"""
Tests for the upload_book_epub MCP tool.

The tool function is invoked directly; the Vercel Blob API is mocked with respx.
"""

import base64

import httpx
import pytest
import respx
from conftest import build_epub

from metabook_py.core.config import settings
from metabook_py.mcp_server import upload_book_epub as upload_fn

BLOB_URL_PATTERN = f"{settings.blob_api_url}/books/"
EPUB_URL = "https://example.com/files/pride.epub"


@pytest.fixture
def blob_token(monkeypatch):
    monkeypatch.setattr(settings, "blob_read_write_token", "vercel_blob_rw_test_token")


def _mock_blob_ok() -> respx.Route:
    return respx.put(url__startswith=BLOB_URL_PATTERN).mock(
        return_value=httpx.Response(
            200,
            json={
                "url": "https://example.private.blob.vercel-storage.com/books/pride-abc.epub",
                "pathname": "books/pride.epub",
            },
        )
    )


# ── Happy paths ────────────────────────────────────────────────────────────────


@respx.mock
async def test_upload_from_base64(blob_token):
    _mock_blob_ok()
    epub = build_epub()

    result = await upload_fn(filename="pride.epub", epub_base64=base64.b64encode(epub).decode())

    assert "error" not in result
    assert result["book"]["title"] == "Pride and Prejudice"
    assert result["book"]["authors"] == ["Jane Austen"]
    assert result["book"]["isbn"] == "9780141439518"
    assert result["blob"]["pathname"] == "books/pride.epub"
    assert result["blob"]["size_bytes"] == len(epub)
    assert result["structure"]["schema"] == "standard_book"
    assert result["spine_document_count"] == 3


@respx.mock
async def test_upload_from_url(blob_token):
    _mock_blob_ok()
    respx.get(EPUB_URL).mock(return_value=httpx.Response(200, content=build_epub()))

    result = await upload_fn(filename="pride.epub", epub_url=EPUB_URL)

    assert "error" not in result
    assert result["structure"]["schema"] == "standard_book"


@respx.mock
async def test_exclude_paragraphs(blob_token):
    _mock_blob_ok()
    result = await upload_fn(
        epub_base64=base64.b64encode(build_epub()).decode(), include_paragraphs=False
    )
    assert all(n.get("paragraphs") is None for n in result["structure"]["nodes"])


# ── Failure modes ──────────────────────────────────────────────────────────────


async def test_requires_exactly_one_source(blob_token):
    neither = await upload_fn()
    both = await upload_fn(epub_base64="aGk=", epub_url=EPUB_URL)
    assert "exactly one" in neither["error"]
    assert "exactly one" in both["error"]


async def test_rejects_invalid_base64(blob_token):
    result = await upload_fn(epub_base64="not base64!!!")
    assert result["error"] == "invalid_base64"


async def test_rejects_invalid_epub(blob_token):
    result = await upload_fn(epub_base64=base64.b64encode(b"not a zip").decode())
    assert result["error"] == "invalid_epub"


async def test_rejects_oversized(blob_token, monkeypatch):
    monkeypatch.setattr(settings, "max_upload_bytes", 10)
    result = await upload_fn(epub_base64=base64.b64encode(build_epub()).decode())
    assert result["error"] == "file_too_large"


@respx.mock
async def test_download_failure(blob_token):
    respx.get(EPUB_URL).mock(return_value=httpx.Response(404))
    result = await upload_fn(epub_url=EPUB_URL)
    assert result["error"] == "download_failed"


@respx.mock
async def test_blob_failure(blob_token):
    respx.put(url__startswith=BLOB_URL_PATTERN).mock(return_value=httpx.Response(403))
    result = await upload_fn(epub_base64=base64.b64encode(build_epub()).decode())
    assert result["error"] == "blob_upload_failed"
