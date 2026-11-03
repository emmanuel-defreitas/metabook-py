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
from metabook_py.services.documents import fetch as url_fetch

BLOB_URL_PATTERN = f"{settings.blob_api_url}/books/"
EPUB_URL = "https://example.com/files/pride.epub"


@pytest.fixture(autouse=True)
def dns(monkeypatch):
    """epub_url downloads resolve hosts first (SSRF check); no real DNS in tests."""
    answers: dict[str, list[str]] = {}

    async def resolve(host: str) -> list[str]:
        return answers.get(host, ["93.184.216.34"])

    monkeypatch.setattr(url_fetch, "resolve_host", resolve)
    return answers


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


# ── epub_url SSRF protection ───────────────────────────────────────────────────


@pytest.mark.parametrize("addr", ["127.0.0.1", "10.0.0.8", "169.254.169.254", "::1", "fd00::2"])
@respx.mock
async def test_url_to_private_address_refused(blob_token, dns, addr):
    dns["internal.example.com"] = [addr]
    route = respx.get(url__startswith="http://internal.example.com")
    result = await upload_fn(epub_url="http://internal.example.com/book.epub")
    assert result["error"] == "url_not_allowed"
    assert not route.called


@pytest.mark.parametrize("url", ["http://127.0.0.1/a.epub", "file:///etc/passwd", "ftp://x/y"])
async def test_url_scheme_and_literal_ip_refused(blob_token, url):
    result = await upload_fn(epub_url=url)
    assert result["error"] == "url_not_allowed"


@respx.mock
async def test_redirect_to_private_address_refused(blob_token, dns):
    dns["metadata.internal"] = ["169.254.169.254"]
    respx.get(EPUB_URL).mock(
        return_value=httpx.Response(302, headers={"location": "http://metadata.internal/"})
    )
    inner = respx.get("http://metadata.internal/")
    result = await upload_fn(epub_url=EPUB_URL)
    assert result["error"] == "url_not_allowed"
    assert not inner.called


@respx.mock
async def test_url_download_capped_while_streaming(blob_token, monkeypatch):
    monkeypatch.setattr(settings, "max_upload_bytes", 10)
    respx.get(EPUB_URL).mock(return_value=httpx.Response(200, content=b"x" * 11))
    result = await upload_fn(epub_url=EPUB_URL)
    assert result["error"] == "file_too_large"
