"""Tests for services/discovery.py — all HTTP calls are mocked with respx."""

import httpx
import pytest
import respx

from metabook_py.core.exceptions import (
    AmbiguousBookError,
    BookNotFoundError,
    UnsupportedFormatError,
)
from metabook_py.services.discovery import GutendexClient

# ── Mock payloads ──────────────────────────────────────────────────────────────

_SINGLE_HIT = {
    "count": 1,
    "results": [
        {
            "id": 1342,
            "title": "Pride and Prejudice",
            "authors": [{"name": "Austen, Jane", "birth_year": 1775, "death_year": 1817}],
            "languages": ["en"],
            "subjects": ["Love stories", "England -- Social life and customs"],
            "formats": {
                "text/plain; charset=utf-8": "https://www.gutenberg.org/files/1342/1342-0.txt",
                "text/html": "https://www.gutenberg.org/files/1342/1342-h/1342-h.htm",
            },
        }
    ],
}

_MULTIPLE_HITS = {
    "count": 3,
    "results": [
        {
            "id": i,
            "title": f"Book {i}",
            "authors": [{"name": f"Author {i}"}],
            "languages": ["en"],
            "formats": {"text/plain; charset=utf-8": f"https://example.com/{i}.txt"},
        }
        for i in range(1, 4)
    ],
}

_NO_HITS = {"count": 0, "results": []}

_NO_TEXT_FORMAT = {
    "count": 1,
    "results": [
        {
            "id": 99,
            "title": "Image Only Book",
            "authors": [],
            "languages": ["en"],
            "subjects": [],
            "formats": {
                "application/epub+zip": "https://example.com/99.epub",
                "image/jpeg": "https://example.com/99.jpg",
            },
        }
    ],
}


# ── Helpers ────────────────────────────────────────────────────────────────────


def gutendex_url() -> str:
    from metabook_py.core.config import settings

    return f"{settings.gutendex_base_url}/books/"


# ── Tests ──────────────────────────────────────────────────────────────────────


class TestGutendexClient:
    @respx.mock
    @pytest.mark.asyncio
    async def test_single_result_by_id(self):
        respx.get(gutendex_url()).mock(return_value=httpx.Response(200, json=_SINGLE_HIT))

        client = GutendexClient()
        book, url, is_html = await client.search(gutenberg_id=1342)

        assert book.gutenberg_id == 1342
        assert book.title == "Pride and Prejudice"
        assert len(book.authors) == 1
        assert book.authors[0].name == "Austen, Jane"
        assert "1342-0.txt" in url
        assert is_html is False

    @respx.mock
    @pytest.mark.asyncio
    async def test_prefers_plain_text_over_html(self):
        respx.get(gutendex_url()).mock(return_value=httpx.Response(200, json=_SINGLE_HIT))

        client = GutendexClient()
        _, url, is_html = await client.search(gutenberg_id=1342)
        assert ".txt" in url
        assert is_html is False

    @respx.mock
    @pytest.mark.asyncio
    async def test_not_found_raises_error(self):
        respx.get(gutendex_url()).mock(return_value=httpx.Response(200, json=_NO_HITS))

        client = GutendexClient()
        with pytest.raises(BookNotFoundError) as exc:
            await client.search(title="xyzzy not a real book")
        assert exc.value.query["title"] == "xyzzy not a real book"

    @respx.mock
    @pytest.mark.asyncio
    async def test_ambiguous_raises_error(self):
        respx.get(gutendex_url()).mock(return_value=httpx.Response(200, json=_MULTIPLE_HITS))

        client = GutendexClient()
        with pytest.raises(AmbiguousBookError) as exc:
            await client.search(title="book")
        assert len(exc.value.matches) == 3

    @respx.mock
    @pytest.mark.asyncio
    async def test_single_result_on_direct_id_no_disambiguation(self):
        """Even if the API returns multiple results, a direct ID lookup
        takes the first result without raising AmbiguousBookError."""
        respx.get(gutendex_url()).mock(return_value=httpx.Response(200, json=_MULTIPLE_HITS))

        client = GutendexClient()
        # gutenberg_id is provided → skip disambiguation logic
        book, _, _ = await client.search(gutenberg_id=1)
        assert book.gutenberg_id == 1

    @respx.mock
    @pytest.mark.asyncio
    async def test_no_text_format_raises_unsupported(self):
        respx.get(gutendex_url()).mock(return_value=httpx.Response(200, json=_NO_TEXT_FORMAT))

        client = GutendexClient()
        with pytest.raises(UnsupportedFormatError):
            await client.search(gutenberg_id=99)

    @respx.mock
    @pytest.mark.asyncio
    async def test_isbn_search_uses_search_param(self):
        """ISBN is passed via ?search= (Gutendex has no dedicated isbn= filter)."""
        route = respx.get(gutendex_url()).mock(return_value=httpx.Response(200, json=_SINGLE_HIT))

        client = GutendexClient()
        await client.search(isbn="9780141439518")

        assert route.called
        request = route.calls.last.request
        assert b"9780141439518" in request.url.query

    @respx.mock
    @pytest.mark.asyncio
    async def test_language_param_forwarded(self):
        route = respx.get(gutendex_url()).mock(return_value=httpx.Response(200, json=_SINGLE_HIT))

        client = GutendexClient()
        await client.search(gutenberg_id=1342, language="fr")

        request = route.calls.last.request
        assert b"fr" in request.url.query
