import httpx
import pytest
import respx

from metabook_py.core.config import settings
from metabook_py.models.book import AuthorInfo, UploadedBookInfo
from metabook_py.services import open_library as ol


@pytest.fixture(autouse=True)
def reset(monkeypatch):
    ol._cache.clear()
    monkeypatch.setattr(ol, "_next_request", 0)
    monkeypatch.setattr(settings, "open_library_enabled", True)


def book(**kwargs):
    return UploadedBookInfo(
        title="Pride and Prejudice", authors=[AuthorInfo(name="Austen, Jane")], **kwargs
    )


@respx.mock
async def test_isbn_enrichment_preserves_source_and_caches():
    route = respx.get("https://openlibrary.org/api/books").respond(
        200,
        json={
            "ISBN:9780141439518": {
                "identifiers": {"openlibrary": ["OL123M"]},
                "cover": {"large": "https://covers.openlibrary.org/b/id/42-L.jpg"},
                "publishers": [{"name": "Catalog publisher"}],
                "publish_date": "2002",
                "number_of_pages": 480,
                "subjects": [{"name": "Fiction"}],
                "classifications": {"dewey_decimal_class": ["823.7"]},
            }
        },
    )
    original = book(isbn="9780141439518", publisher="Source publisher", subjects=["Source"])
    result = await ol.enrich_uploaded_book(original)
    assert result.publisher == "Source publisher"
    assert result.subjects == ["Source"]
    assert result.date == "2002"
    assert result.number_of_pages == 480
    assert result.open_library.edition_id == "OL123M"
    assert result.open_library.cover_id == 42
    assert result.open_library.dewey_decimal == ["823.7"]
    await ol.enrich_uploaded_book(original)
    assert route.call_count == 1
    assert original.open_library is None


@respx.mock
async def test_title_author_match_is_work_only():
    respx.get("https://openlibrary.org/search.json").respond(
        200,
        json={
            "numFound": 1,
            "docs": [
                {
                    "key": "/works/OL123W",
                    "title": "Pride and Prejudice",
                    "author_name": ["Jane Austen"],
                    "cover_i": 42,
                    "subject": ["Fiction"],
                    "number_of_pages": 999,
                }
            ],
        },
    )
    result = await ol.enrich_uploaded_book(book())
    assert result.open_library.work_id == "OL123W"
    assert result.open_library.match_method == "title_author"
    assert result.number_of_pages is None


@pytest.mark.parametrize(
    "docs",
    [
        [],
        [{"key": "/works/OL1W", "title": "Pride and Prejudice", "author_name": ["Other Author"]}],
        [
            {"key": "/works/OL1W", "title": "Pride and Prejudice", "author_name": ["Jane Austen"]},
            {"key": "/works/OL2W", "title": "Pride and Prejudice", "author_name": ["Jane Austen"]},
        ],
    ],
)
@respx.mock
async def test_ambiguous_or_wrong_match_is_ignored(docs):
    respx.get("https://openlibrary.org/search.json").respond(200, json={"docs": docs})
    original = book()
    assert await ol.enrich_uploaded_book(original) is original


@pytest.mark.parametrize("status", [404, 429, 503])
@respx.mock
async def test_failure_is_nonfatal(status):
    respx.get("https://openlibrary.org/search.json").respond(status)
    original = book()
    assert await ol.enrich_uploaded_book(original) is original


@respx.mock
async def test_timeout_is_nonfatal():
    respx.get("https://openlibrary.org/search.json").mock(side_effect=httpx.ReadTimeout("timeout"))
    original = book()
    assert await ol.enrich_uploaded_book(original) is original
