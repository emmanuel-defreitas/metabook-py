"""Gutendex catalog search stays separate from book analysis and persistence."""

from unittest.mock import AsyncMock

import httpx
import pytest
import respx

from metabook_py.core.config import settings
from metabook_py.main import app
from metabook_py.routers import books

GUTENDEX_URL = f"{settings.gutendex_base_url}/books/"
BOOK = {
    "id": 1342,
    "title": "Pride and Prejudice",
    "authors": [{"name": "Austen, Jane"}],
    "languages": ["en", "fr"],
    # Search metadata remains useful even without a downloadable text format.
    "formats": {},
}


@pytest.fixture
async def client():
    async with httpx.AsyncClient(
        transport=httpx.ASGITransport(app=app), base_url="http://testserver"
    ) as client:
        yield client


@respx.mock
async def test_search_pages_metadata_without_scanning_or_saving(client, monkeypatch):
    fetch = AsyncMock()
    store = AsyncMock()
    monkeypatch.setattr(books, "fetch_book_text", fetch)
    monkeypatch.setattr(books, "get_upload_store", lambda: store)
    route = respx.get(GUTENDEX_URL).mock(
        return_value=httpx.Response(
            200,
            json={
                "count": 70,
                "next": "https://gutendex.com/books/?page=3",
                "previous": "https://gutendex.com/books/?page=1",
                "results": [BOOK],
            },
        )
    )
    response = await client.get(
        "/api/books/search", params={"q": " Austen ", "page": 2, "language": "en,fr"}
    )
    assert response.status_code == 200
    assert response.json() == {
        "count": 70,
        "page": 2,
        "next_page": 3,
        "previous_page": 1,
        "results": [
            {
                "gutenberg_id": 1342,
                "title": "Pride and Prejudice",
                "authors": ["Austen, Jane"],
                "language": "en,fr",
            }
        ],
    }
    assert dict(route.calls.last.request.url.params) == {
        "search": "Austen",
        "page": "2",
        "languages": "en,fr",
    }
    fetch.assert_not_called()
    store.record_gutenberg_book.assert_not_called()
    store.record_scan.assert_not_called()


@respx.mock
async def test_empty_search_is_a_successful_empty_page(client):
    respx.get(GUTENDEX_URL).mock(return_value=httpx.Response(200, json={"count": 0, "results": []}))
    response = await client.get("/api/books/search", params={"q": "no such book"})
    assert response.status_code == 200
    assert response.json() == {
        "count": 0,
        "page": 1,
        "next_page": None,
        "previous_page": None,
        "results": [],
    }


@respx.mock
async def test_selected_book_without_text_returns_actionable_error(client):
    respx.get(f"{GUTENDEX_URL}1342/").mock(return_value=httpx.Response(200, json=BOOK))
    response = await client.get("/api/books/structure", params={"gutenberg_id": 1342})
    assert response.status_code == 422
    assert response.json()["detail"]["error"] == "unsupported_format"


@pytest.mark.parametrize(
    ("params", "upstream"),
    [
        ({"gutenberg_id": 1342}, {"ids": "1342"}),
        ({"isbn": "9780141439518"}, {"search": "9780141439518"}),
    ],
)
@respx.mock
async def test_search_identifier_mapping_and_all_languages(client, params, upstream):
    by_id = "gutenberg_id" in params
    route = respx.get(f"{GUTENDEX_URL}1342/" if by_id else GUTENDEX_URL).mock(
        return_value=httpx.Response(200, json=BOOK if by_id else {"count": 1, "results": [BOOK]})
    )
    response = await client.get("/api/books/search", params=params)
    assert response.status_code == 200
    assert dict(route.calls.last.request.url.params) == ({} if by_id else {**upstream, "page": "1"})


@respx.mock
async def test_missing_gutenberg_id_is_an_empty_search(client):
    respx.get(f"{GUTENDEX_URL}999999999/").mock(
        return_value=httpx.Response(404, json={"detail": "Not found"})
    )
    response = await client.get("/api/books/search", params={"gutenberg_id": 999999999})
    assert response.status_code == 200
    assert response.json()["count"] == 0
    assert response.json()["results"] == []


@pytest.mark.parametrize(
    "params", [{}, {"q": "   "}, {"q": "Austen", "page": 0}, {"gutenberg_id": -1}]
)
@respx.mock
async def test_invalid_search_is_rejected_before_upstream(client, params):
    response = await client.get("/api/books/search", params=params)
    assert response.status_code == 422
    assert not respx.calls


@pytest.mark.parametrize(
    ("error", "status"),
    [(httpx.ReadTimeout("timeout"), 504), (httpx.ConnectError("unreachable"), 502)],
)
@respx.mock
async def test_upstream_outages_keep_http_error_mapping(client, error, status):
    respx.get(GUTENDEX_URL).mock(side_effect=error)
    response = await client.get("/api/books/search", params={"q": "Austen"})
    assert response.status_code == status
    assert response.json()["detail"]["error"] == "gutendex_unreachable"


@pytest.mark.parametrize(
    "upstream",
    [
        httpx.Response(500),
        httpx.Response(200, text="bad json"),
        httpx.Response(200, json={"results": [{}]}),
    ],
)
@respx.mock
async def test_bad_upstream_returns_502(client, upstream):
    respx.get(GUTENDEX_URL).mock(return_value=upstream)
    response = await client.get("/api/books/search", params={"q": "Austen"})
    assert response.status_code == 502
    assert response.json()["detail"]["error"] == "gutendex_unreachable"
