"""
Endpoint tests for POST /api/books/upload.

The Vercel Blob API is mocked with respx; EPUBs are built in memory.
"""

import httpx
import pytest
import respx
from conftest import build_epub

from metabook_py.core.config import settings
from metabook_py.main import app

BLOB_URL_PATTERN = f"{settings.blob_api_url}/books/"


@pytest.fixture
def client() -> httpx.AsyncClient:
    return httpx.AsyncClient(transport=httpx.ASGITransport(app=app), base_url="http://testserver")


@pytest.fixture
def blob_token(monkeypatch):
    monkeypatch.setattr(settings, "blob_read_write_token", "vercel_blob_rw_test_token")


def _mock_blob_ok(route_suffix: str = "book.epub") -> respx.Route:
    return respx.put(url__startswith=BLOB_URL_PATTERN).mock(
        return_value=httpx.Response(
            200,
            json={
                "url": f"https://example.public.blob.vercel-storage.com/books/{route_suffix}",
                "pathname": f"books/{route_suffix}",
                "contentType": "application/epub+zip",
            },
        )
    )


# ── Happy path ─────────────────────────────────────────────────────────────────


@respx.mock
async def test_upload_returns_structure(client, blob_token):
    _mock_blob_ok()
    epub = build_epub()

    resp = await client.post(
        "/api/books/upload",
        files={"file": ("pride.epub", epub, "application/epub+zip")},
    )

    assert resp.status_code == 201
    body = resp.json()
    assert body["book"]["title"] == "Pride and Prejudice"
    assert body["book"]["authors"][0]["name"] == "Jane Austen"
    assert body["book"]["isbn"] == "9780141439518"
    assert body["book"]["source"] == "upload"
    assert body["blob"]["pathname"].startswith("books/")
    assert body["blob"]["size_bytes"] == len(epub)
    assert body["structure"]["schema"] == "standard_book"
    assert body["structure"]["summary"]["total_words"] > 0
    assert body["meta"]["spine_document_count"] == 3


@respx.mock
async def test_upload_sends_bearer_token_and_folder(client, blob_token):
    route = _mock_blob_ok()
    await client.post(
        "/api/books/upload",
        files={"file": ("my book (1).epub", build_epub(), "application/epub+zip")},
    )
    request = route.calls.last.request
    assert request.headers["authorization"] == "Bearer vercel_blob_rw_test_token"
    assert request.headers["x-vercel-blob-access"] == settings.blob_access
    assert "/books/" in str(request.url)


@respx.mock
async def test_upload_exclude_paragraphs(client, blob_token):
    _mock_blob_ok()
    resp = await client.post(
        "/api/books/upload?include_paragraphs=false",
        files={"file": ("pride.epub", build_epub(), "application/epub+zip")},
    )
    assert resp.status_code == 201
    for node in resp.json()["structure"]["nodes"]:
        assert node.get("paragraphs") is None


# ── Failure modes ──────────────────────────────────────────────────────────────


async def test_rejects_non_epub_extension(client, blob_token):
    resp = await client.post(
        "/api/books/upload",
        files={"file": ("notes.txt", b"hello", "text/plain")},
    )
    assert resp.status_code == 400
    assert resp.json()["detail"]["error"] == "invalid_file"


async def test_rejects_invalid_epub_bytes(client, blob_token):
    resp = await client.post(
        "/api/books/upload",
        files={"file": ("fake.epub", b"not a zip", "application/epub+zip")},
    )
    assert resp.status_code == 400
    assert resp.json()["detail"]["error"] == "invalid_epub"


async def test_rejects_oversized_file(client, blob_token, monkeypatch):
    monkeypatch.setattr(settings, "max_upload_bytes", 10)
    resp = await client.post(
        "/api/books/upload",
        files={"file": ("pride.epub", build_epub(), "application/epub+zip")},
    )
    assert resp.status_code == 413
    assert resp.json()["detail"]["error"] == "file_too_large"


async def test_missing_token_yields_502(client, monkeypatch):
    monkeypatch.setattr(settings, "blob_read_write_token", "")
    resp = await client.post(
        "/api/books/upload",
        files={"file": ("pride.epub", build_epub(), "application/epub+zip")},
    )
    assert resp.status_code == 502
    assert resp.json()["detail"]["error"] == "blob_upload_failed"


@respx.mock
async def test_blob_error_yields_502(client, blob_token):
    respx.put(url__startswith=BLOB_URL_PATTERN).mock(return_value=httpx.Response(403))
    resp = await client.post(
        "/api/books/upload",
        files={"file": ("pride.epub", build_epub(), "application/epub+zip")},
    )
    assert resp.status_code == 502
    assert resp.json()["detail"]["error"] == "blob_upload_failed"
