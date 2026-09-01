"""
Endpoint tests for the optional ?tokenizer= query parameter.

The tokenizer resolver is stubbed at the router boundary (no network); the
Vercel Blob API is mocked with respx as in test_upload.py.
"""

import httpx
import pytest
import respx
from conftest import build_epub

from metabook_py.core.config import settings
from metabook_py.core.exceptions import TokenizerNotFoundError, TokenizerUnavailableError
from metabook_py.main import app
from metabook_py.services.tokenizers import TokenEncoder

BLOB_URL_PATTERN = f"{settings.blob_api_url}/books/"


@pytest.fixture
def client() -> httpx.AsyncClient:
    return httpx.AsyncClient(transport=httpx.ASGITransport(app=app), base_url="http://testserver")


@pytest.fixture
def blob_token(monkeypatch):
    monkeypatch.setattr(settings, "blob_read_write_token", "vercel_blob_rw_test_token")


@pytest.fixture
def stub_resolver(monkeypatch):
    """Resolve any tokenizer name to a whitespace-counting stub encoder."""

    def fake_get_encoder(name: str) -> TokenEncoder:
        return TokenEncoder(name=name, vocab_size=5, encode=lambda text: len(text.split()))

    monkeypatch.setattr("metabook_py.routers.books.get_encoder", fake_get_encoder)


def _mock_blob_ok() -> respx.Route:
    return respx.put(url__startswith=BLOB_URL_PATTERN).mock(
        return_value=httpx.Response(
            200,
            json={
                "url": "https://example.public.blob.vercel-storage.com/books/book.epub",
                "pathname": "books/book.epub",
                "contentType": "application/epub+zip",
            },
        )
    )


def _upload(client, **params):
    return client.post(
        "/api/books/upload",
        params=params,
        files={"file": ("pride.epub", build_epub(), "application/epub+zip")},
    )


# ── Token counts present when a tokenizer is given ─────────────────────────────


@respx.mock
async def test_tokenizer_adds_counts_and_metadata(client, blob_token, stub_resolver):
    _mock_blob_ok()
    resp = await _upload(client, tokenizer="stub-tokenizer")

    assert resp.status_code == 201
    body = resp.json()

    summary = body["structure"]["summary"]
    assert summary["total_tokens"] > 0

    chapters = body["structure"]["nodes"]
    for chapter in chapters:
        assert chapter["total_tokens"] == sum(p["token_count"] for p in chapter["paragraphs"])
    assert summary["total_tokens"] == sum(c["total_tokens"] for c in chapters)

    # The resolved scheme is echoed alongside the counts.
    assert body["meta"]["tokenizer"] == {"name": "stub-tokenizer", "vocab_size": 5}


@respx.mock
async def test_no_tokenizer_leaves_response_unchanged(client, blob_token, stub_resolver):
    _mock_blob_ok()
    resp = await _upload(client)

    assert resp.status_code == 201
    body = resp.json()

    assert "total_tokens" not in body["structure"]["summary"]
    for chapter in body["structure"]["nodes"]:
        assert "total_tokens" not in chapter
        for para in chapter["paragraphs"]:
            assert "token_count" not in para
    assert "tokenizer" not in body["meta"]


# ── Error mapping ──────────────────────────────────────────────────────────────


@pytest.fixture
def resolver_raising(monkeypatch):
    def _install(exc: Exception):
        def fake_get_encoder(name: str):
            raise exc

        monkeypatch.setattr("metabook_py.routers.books.get_encoder", fake_get_encoder)

    return _install


async def test_unknown_tokenizer_is_422(client, resolver_raising):
    resolver_raising(TokenizerNotFoundError("no-such/tokenizer", "repository not found"))
    resp = await _upload(client, tokenizer="no-such/tokenizer")

    assert resp.status_code == 422
    detail = resp.json()["detail"]
    assert detail["error"] == "tokenizer_not_found"
    assert detail["tokenizer"] == "no-such/tokenizer"


async def test_unreachable_tokenizer_is_503(client, resolver_raising):
    resolver_raising(TokenizerUnavailableError("bert-base-uncased", "connection reset"))
    resp = await _upload(client, tokenizer="bert-base-uncased")

    assert resp.status_code == 503
    detail = resp.json()["detail"]
    assert detail["error"] == "tokenizer_unavailable"
    assert detail["tokenizer"] == "bert-base-uncased"


async def test_structure_endpoint_maps_tokenizer_errors_too(client, resolver_raising):
    # The encoder is resolved before any Gutendex traffic, so no HTTP mock
    # is needed to observe the error mapping on GET /structure.
    resolver_raising(TokenizerNotFoundError("no-such/tokenizer", "repository not found"))
    resp = await client.get(
        "/api/books/structure",
        params={"gutenberg_id": 1342, "tokenizer": "no-such/tokenizer"},
    )

    assert resp.status_code == 422
    assert resp.json()["detail"]["tokenizer"] == "no-such/tokenizer"
