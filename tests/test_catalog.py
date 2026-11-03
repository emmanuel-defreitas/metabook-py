"""Live catalog contracts, provider adapters, and preflight validation (no real network)."""

from copy import deepcopy
from datetime import UTC, datetime
from unittest.mock import AsyncMock, Mock

import httpx
import pytest
import respx
from pydantic import SecretStr

from metabook_py.core.config import Settings, settings
from metabook_py.core.exceptions import CatalogProviderError
from metabook_py.main import app
from metabook_py.routers import books
from metabook_py.services import catalog
from metabook_py.services.catalog_providers import GutendexCatalogClient, OpenAlexCatalogClient

ENDPOINT = "/api/books/catalog/search"
GUTENDEX = "https://gutendex.test/books/"
OPENALEX = "https://openalex.test/works"
BOOK = {
    "id": 1342,
    "title": "Pride and Prejudice",
    "authors": [{"name": "Austen, Jane"}],
    "languages": ["en", "fr"],
    "subjects": ["Courtship -- Fiction", "England -- Fiction"],
    "formats": {"text/plain": "http://unsafe.test/book.txt"},
}
WORK = {
    "id": "https://openalex.org/W12345",
    "title": "A scientific work",
    "authorships": [{"author": {"display_name": "Researcher"}}],
    "language": "en",
    "publication_date": "2026-10-01",
    "type": "article",
    "primary_topic": {"domain": {"id": "https://openalex.org/domains/3"}},
    "primary_location": {"landing_page_url": "javascript:alert(1)"},
    "doi": "https://evil.test/action",
}


@pytest.fixture(autouse=True)
def catalog_settings(monkeypatch):
    monkeypatch.setattr(settings, "gutendex_base_url", "https://gutendex.test/")
    monkeypatch.setattr(settings, "openalex_base_url", "https://openalex.test/")
    monkeypatch.setattr(settings, "openalex_api_key", None)


@pytest.fixture
async def client():
    async with httpx.AsyncClient(
        transport=httpx.ASGITransport(app=app), base_url="http://testserver"
    ) as client:
        yield client


def gutendex_page(count=1, results=None, *, next_page=None):
    return {"count": count, "results": [BOOK] if results is None else results, "next": next_page}


def openalex_page(count=1, results=None):
    return {"meta": {"count": count}, "results": [WORK] if results is None else results}


@respx.mock
async def test_default_gutendex_contract_and_no_scan_save(client, monkeypatch):
    forbidden = Mock(side_effect=AssertionError("Catalog search must not scan or save"))
    fetch = AsyncMock(side_effect=AssertionError("Catalog search must not download text"))
    monkeypatch.setattr(books, "fetch_book_text", fetch)
    for name in ("get_upload_store", "detect_schema", "build_structure_tree", "get_encoder"):
        monkeypatch.setattr(books, name, forbidden)
    route = respx.get(GUTENDEX).mock(return_value=httpx.Response(200, json=gutendex_page()))
    response = await client.get(ENDPOINT, params={"q": " Austen "})
    assert response.status_code == 200
    assert response.json() == {
        "count": 1,
        "page": 1,
        "next_page": None,
        "previous_page": None,
        "warnings": [],
        "results": [
            {
                "source": "gutendex",
                "id": "1342",
                "gutenberg_id": 1342,
                "title": "Pride and Prejudice",
                "authors": ["Austen, Jane"],
                "language": "en,fr",
                "publication_date": None,
                "kind": "book",
                "categories": ["subject:Courtship -- Fiction", "subject:England -- Fiction"],
                "url": "https://www.gutenberg.org/ebooks/1342",
            }
        ],
    }
    assert dict(route.calls.last.request.url.params) == {"search": "Austen", "page": "1"}
    forbidden.assert_not_called()
    fetch.assert_not_called()


@respx.mock
async def test_openalex_contract_canonical_url_no_scan_save(client, monkeypatch):
    forbidden = Mock(side_effect=AssertionError("Catalog search must not scan or save"))
    monkeypatch.setattr(books, "get_upload_store", forbidden)
    monkeypatch.setattr(books, "fetch_book_text", forbidden)
    route = respx.get(OPENALEX).mock(return_value=httpx.Response(200, json=openalex_page()))
    response = await client.get(ENDPOINT, params={"q": " science ", "source": "openalex"})
    assert response.status_code == 200
    assert response.json()["results"] == [
        {
            "source": "openalex",
            "id": "W12345",
            "gutenberg_id": None,
            "title": "A scientific work",
            "authors": ["Researcher"],
            "language": "en",
            "publication_date": "2026-10-01",
            "kind": "article",
            "categories": ["domain:3"],
            "url": "https://openalex.org/W12345",
        }
    ]
    request = route.calls.last.request
    assert dict(request.url.params) == {"search": "science", "page": "1", "per_page": "32"}
    assert "authorization" not in request.headers
    forbidden.assert_not_called()


@respx.mock
async def test_openalex_or_filters_and_across_dimension_and(client, monkeypatch):
    class Clock(datetime):
        @classmethod
        def now(cls, tz=None):
            assert tz == UTC
            return cls(2026, 10, 8, 0, 1, tzinfo=UTC)

    monkeypatch.setattr(catalog, "datetime", Clock)
    route = respx.get(OPENALEX).mock(return_value=httpx.Response(200, json=openalex_page(70)))
    response = await client.get(
        ENDPOINT,
        params=[
            ("q", "biology"),
            ("source", "openalex"),
            ("page", "2"),
            ("category", "domain:4"),
            ("category", "domain:1"),
            ("category", "domain:1"),
            ("kind", "academic"),
            ("kind", "book"),
            ("kind", "academic"),
            ("kind", "other"),
            ("kind", "other"),
            ("date", "today"),
            ("date", "7days"),
            ("date", "30days"),
        ],
    )
    assert response.status_code == 200
    assert dict(route.calls.last.request.url.params) == {
        "search": "biology",
        "page": "2",
        "per_page": "32",
        "filter": "primary_topic.domain.id:1|4,"
        "type:book|article|preprint|review|dissertation|report|conference-paper|other,"
        "from_publication_date:2026-09-09,to_publication_date:2026-10-08",
    }
    body = response.json()
    assert (body["count"], body["page"], body["next_page"], body["previous_page"]) == (70, 2, 3, 1)


@pytest.mark.parametrize(
    ("keys", "start"),
    [
        (["today"], "2026-10-08"),
        (["7days"], "2026-10-02"),
        (["30days"], "2026-09-09"),
        (["year"], "2025-10-09"),
        (["today", "year", "30days"], "2025-10-09"),
    ],
)
@respx.mock
async def test_utc_publication_date_ranges(client, monkeypatch, keys, start):
    class Clock(datetime):
        @classmethod
        def now(cls, tz=None):
            assert tz == UTC
            return cls(2026, 10, 8, 23, 59, tzinfo=UTC)

    monkeypatch.setattr(catalog, "datetime", Clock)
    route = respx.get(OPENALEX).mock(return_value=httpx.Response(200, json=openalex_page()))
    response = await client.get(
        ENDPOINT,
        params=[
            ("q", "science"),
            ("source", "openalex"),
            *[("date", key) for key in keys],
        ],
    )
    assert response.status_code == 200
    assert route.calls.last.request.url.params["filter"] == (
        f"from_publication_date:{start},to_publication_date:2026-10-08"
    )


def test_date_or_selects_actual_widest_range_across_year_boundary():
    assert catalog._date_range({"year", "30days"}, datetime(2026, 1, 2).date()) == (
        "2025-01-03",
        "2026-01-02",
    )


@pytest.mark.parametrize("domain", ["1", "2", "3", "4"])
@respx.mock
async def test_domain_keys_forward_verified_ids(client, domain):
    work = {**WORK, "primary_topic": {"domain": {"id": f"https://openalex.org/domains/{domain}"}}}
    route = respx.get(OPENALEX).mock(
        return_value=httpx.Response(200, json=openalex_page(results=[work]))
    )
    response = await client.get(
        ENDPOINT,
        params={
            "q": "science",
            "source": "openalex",
            "category": f"domain:{domain}",
        },
    )
    assert response.status_code == 200
    assert route.calls.last.request.url.params["filter"] == f"primary_topic.domain.id:{domain}"
    assert response.json()["results"][0]["categories"] == [f"domain:{domain}"]


@respx.mock
async def test_single_subject_forwarded_with_book_only(client):
    route = respx.get(GUTENDEX).mock(return_value=httpx.Response(200, json=gutendex_page()))
    response = await client.get(
        ENDPOINT,
        params=[
            ("q", "Austen"),
            ("category", "subject:Courtship -- Fiction"),
            ("category", "subject:Courtship -- Fiction"),
            ("kind", "book"),
        ],
    )
    assert response.status_code == 200
    assert dict(route.calls.last.request.url.params) == {
        "search": "Austen",
        "topic": "Courtship -- Fiction",
        "page": "1",
    }


@pytest.mark.parametrize(
    ("extra", "message"),
    [
        ([("date", "today")], "Gutendex has no publication dates"),
        (
            [("date", "7days"), ("source", "gutendex"), ("source", "openalex")],
            "Gutendex has no publication dates",
        ),
        ([("category", "subject:A"), ("category", "subject:B")], "multiple subject"),
        ([("category", "domain:1")], "Gutendex does not support"),
        ([("source", "openalex"), ("category", "subject:Fiction")], "OpenAlex does not support"),
        (
            [("source", "openalex"), ("source", "gutendex"), ("category", "subject:Fiction")],
            "OpenAlex does not support",
        ),
        ([("kind", "academic")], "Gutendex supports"),
        ([("kind", "other")], "unsupported kind(s): other"),
        ([("kind", "book"), ("kind", "other")], "unsupported kind(s): other"),
        (
            [("kind", "other"), ("source", "openalex"), ("source", "gutendex")],
            "unsupported kind(s): other",
        ),
        ([("kind", "book"), ("kind", "academic")], "Gutendex supports"),
        ([("kind", "canon")], "saved-only"),
        ([("kind", "corpus"), ("source", "openalex")], "saved-only"),
        ([("kind", "collection"), ("source", "openalex")], "saved-only"),
        ([("source", "upload")], "saved-only"),
        ([("source", "kindle")], "not connected"),
        ([("source", "other")], "Unknown source"),
        ([("source", "")], "Unknown source"),
        ([("date", "yesterday")], "Unknown date"),
        ([("kind", "dataset"), ("source", "openalex")], "Unknown kind"),
        ([("category", "fiction")], "invalid category"),
        ([("category", "domain:5")], "invalid category"),
        ([("category", "domain:01")], "invalid category"),
        ([("category", "domain:1|2")], "invalid category"),
        ([("category", "subject:")], "invalid category"),
        ([("category", "subject:  ")], "invalid category"),
        ([("category", "subject: Fiction ")], "invalid category"),
        ([("isbn", "9780141439518"), ("source", "openalex")], "ISBN or Gutenberg ID"),
        (
            [("gutenberg_id", "1342"), ("source", "openalex"), ("source", "gutendex")],
            "ISBN or Gutenberg ID",
        ),
        ([("page", "313"), ("source", "gutendex"), ("source", "openalex")], "10,000-result limit"),
    ],
)
@respx.mock
async def test_unsupported_filters_fail_all_providers_before_network(client, extra, message):
    response = await client.get(ENDPOINT, params=[("q", "science"), *extra])
    assert response.status_code == 422
    assert message in response.json()["detail"]
    assert not respx.calls


@pytest.mark.parametrize(
    "params", [{}, {"q": "  "}, {"isbn": "  "}, {"q": "x", "page": 0}, {"gutenberg_id": -1}]
)
@respx.mock
async def test_invalid_query_preflight(client, params):
    response = await client.get(ENDPOINT, params=params)
    assert response.status_code == 422
    assert not respx.calls


@pytest.mark.parametrize(
    ("count_g", "count_o", "next_g", "expected_next"),
    [
        (70, 90, "https://evil.test/never-follow?page=3", 3),
        (34, 90, None, 3),
        (70, 34, "https://evil.test/never-follow?page=3", 3),
        (34, 34, None, None),
    ],
)
@respx.mock
async def test_sources_aggregate_same_page_stable_order_and_counts(
    client, count_g, count_o, next_g, expected_next
):
    gutendex = respx.get(GUTENDEX).mock(
        return_value=httpx.Response(200, json=gutendex_page(count_g, next_page=next_g))
    )
    openalex = respx.get(OPENALEX).mock(
        return_value=httpx.Response(200, json=openalex_page(count_o))
    )
    response = await client.get(
        ENDPOINT,
        params=[
            ("q", "science"),
            ("source", "openalex"),
            ("source", "gutendex"),
            ("source", "openalex"),
            ("page", "2"),
            ("kind", "book"),
        ],
    )
    assert response.status_code == 200
    body = response.json()
    assert body["count"] == count_g + count_o
    assert body["page"] == 2
    assert body["previous_page"] == 1
    assert body["next_page"] == expected_next
    assert [result["source"] for result in body["results"]] == ["gutendex", "openalex"]
    assert gutendex.call_count == openalex.call_count == 1
    assert gutendex.calls.last.request.url.params["page"] == "2"
    assert openalex.calls.last.request.url.params["page"] == "2"
    assert openalex.calls.last.request.url.params["filter"] == "type:book"


@respx.mock
async def test_exhausted_gutendex_recovers_total_without_misleading_partial_count(client):
    page3 = respx.get(GUTENDEX, params={"page": "3", "search": "science"}).mock(
        return_value=httpx.Response(404, json={"detail": "Invalid page."})
    )
    page1 = respx.get(GUTENDEX, params={"page": "1", "search": "science"}).mock(
        return_value=httpx.Response(200, json=gutendex_page(1))
    )
    respx.get(OPENALEX).mock(return_value=httpx.Response(200, json=openalex_page(100)))
    response = await client.get(
        ENDPOINT,
        params=[
            ("q", "science"),
            ("page", "3"),
            ("source", "openalex"),
            ("source", "gutendex"),
        ],
    )
    assert response.status_code == 200
    body = response.json()
    assert body["count"] == 101
    assert body["next_page"] == 4
    assert body["previous_page"] == 2
    assert [item["source"] for item in body["results"]] == ["openalex"]
    assert page3.called and page1.called


@respx.mock
async def test_exhausted_openalex_preserves_total(client):
    respx.get(GUTENDEX).mock(
        return_value=httpx.Response(200, json=gutendex_page(100, next_page="https://evil.test/"))
    )
    respx.get(OPENALEX).mock(return_value=httpx.Response(200, json=openalex_page(1, [])))
    response = await client.get(
        ENDPOINT,
        params=[
            ("q", "science"),
            ("page", "3"),
            ("source", "openalex"),
            ("source", "gutendex"),
        ],
    )
    assert response.status_code == 200
    assert response.json()["count"] == 101
    assert response.json()["next_page"] == 4
    assert [item["source"] for item in response.json()["results"]] == ["gutendex"]


@pytest.mark.parametrize("provider", ["gutendex", "openalex"])
@respx.mock
async def test_empty_provider_pages(client, provider):
    payload = gutendex_page(0, []) if provider == "gutendex" else openalex_page(0, [])
    respx.get(GUTENDEX if provider == "gutendex" else OPENALEX).mock(
        return_value=httpx.Response(200, json=payload)
    )
    response = await client.get(ENDPOINT, params={"q": "unknown", "source": provider})
    assert response.status_code == 200
    assert response.json() == {
        "count": 0,
        "page": 1,
        "next_page": None,
        "previous_page": None,
        "results": [],
        "warnings": [],
    }


@pytest.mark.parametrize("page", [1, 2])
@respx.mock
async def test_gutenberg_id_direct_mapping_precedence_and_paging(client, page):
    route = respx.get(f"{GUTENDEX}1342/").mock(return_value=httpx.Response(200, json=BOOK))
    response = await client.get(
        ENDPOINT,
        params={
            "q": "ignored",
            "isbn": "ignored",
            "gutenberg_id": 1342,
            "page": page,
        },
    )
    assert response.status_code == 200
    assert not route.calls.last.request.url.params
    assert response.json()["count"] == 1
    assert response.json()["next_page"] is None
    assert len(response.json()["results"]) == (1 if page == 1 else 0)


@respx.mock
async def test_isbn_remains_best_effort_keyword_search(client):
    route = respx.get(GUTENDEX).mock(return_value=httpx.Response(200, json=gutendex_page()))
    response = await client.get(ENDPOINT, params={"q": "ignored", "isbn": " 9780141439518 "})
    assert response.status_code == 200
    assert route.calls.last.request.url.params["search"] == "9780141439518"


@respx.mock
async def test_id_plus_subject_applies_both_filters(client):
    route = respx.get(GUTENDEX).mock(return_value=httpx.Response(200, json=gutendex_page(0, [])))
    response = await client.get(
        ENDPOINT, params={"gutenberg_id": 1342, "category": "subject:Science"}
    )
    assert response.status_code == 200
    assert response.json()["count"] == 0
    assert dict(route.calls.last.request.url.params) == {
        "ids": "1342",
        "topic": "Science",
        "page": "1",
    }


@respx.mock
async def test_missing_gutenberg_id_is_empty(client):
    respx.get(f"{GUTENDEX}1342/").mock(return_value=httpx.Response(404))
    response = await client.get(ENDPOINT, params={"gutenberg_id": 1342})
    assert response.status_code == 200
    assert response.json()["count"] == 0


@pytest.mark.parametrize("provider", ["gutendex", "openalex"])
@pytest.mark.parametrize(
    ("failure", "status", "message"),
    [
        (httpx.ReadTimeout("private secret"), 504, "timed out"),
        (httpx.ConnectError("private secret"), 502, "Could not reach"),
        (httpx.Response(500, text="private secret"), 502, "HTTP 500"),
        (httpx.Response(429, text="private secret"), 429, "rate limit"),
        (httpx.Response(200, text="private secret"), 502, "invalid JSON"),
    ],
)
@respx.mock
async def test_provider_errors_abort_aggregation_and_are_safe(
    client, provider, failure, status, message
):
    for name, url, payload in (
        ("gutendex", GUTENDEX, gutendex_page()),
        ("openalex", OPENALEX, openalex_page()),
    ):
        route = respx.get(url)
        if name != provider:
            route.mock(return_value=httpx.Response(200, json=payload))
        elif isinstance(failure, Exception):
            route.mock(side_effect=failure)
        else:
            route.mock(return_value=failure)
    response = await client.get(
        ENDPOINT,
        params=[
            ("q", "science"),
            ("source", "gutendex"),
            ("source", "openalex"),
        ],
    )
    assert response.status_code == status
    assert response.json()["detail"]["provider"] == provider
    assert message in response.json()["detail"]["message"]
    assert "private secret" not in response.text
    assert "results" not in response.json()
    if provider == "openalex" and status == 429:
        assert "free server-side OpenAlex API key" in response.text


@respx.mock
async def test_openalex_optional_secret_only_in_auth_header(client, monkeypatch):
    monkeypatch.setattr(settings, "openalex_api_key", SecretStr("server-secret-key"))
    route = respx.get(OPENALEX).mock(return_value=httpx.Response(200, json=openalex_page()))
    response = await client.get(ENDPOINT, params={"q": "science", "source": "openalex"})
    assert response.status_code == 200
    request = route.calls.last.request
    assert request.headers["authorization"] == "Bearer server-secret-key"
    assert "api_key" not in request.url.params
    assert "server-secret-key" not in str(request.url)
    assert "server-secret-key" not in response.text
    assert "server-secret-key" not in repr(settings)


@pytest.mark.parametrize("status", [401, 403])
@respx.mock
async def test_openalex_bad_credentials_actionable_and_not_leaked(client, monkeypatch, status):
    monkeypatch.setattr(settings, "openalex_api_key", SecretStr("server-secret-key"))
    respx.get(OPENALEX).mock(return_value=httpx.Response(status, text="server-secret-key"))
    response = await client.get(ENDPOINT, params={"q": "science", "source": "openalex"})
    assert response.status_code == 502
    assert "OPENALEX_API_KEY" in response.text
    assert "server-secret-key" not in response.text


def test_optional_secret_settings_from_environment(monkeypatch):
    monkeypatch.setenv("OPENALEX_API_KEY", "environment-secret")
    configured = Settings(_env_file=None)
    assert isinstance(configured.openalex_api_key, SecretStr)
    assert configured.openalex_api_key.get_secret_value() == "environment-secret"
    assert "environment-secret" not in configured.model_dump_json()


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("id", "https://evil.test/W12345"),
        ("id", "https://openalex.org/W12345?redirect=evil"),
        ("id", "https://openalex.org/W12345/../../admin"),
        ("id", "http://openalex.org/W12345"),
        ("id", "A12345"),
        ("id", "W0"),
        ("id", 12345),
        ("title", None),
        ("title", []),
        ("title", ""),
        ("authorships", None),
        ("authorships", [{}]),
        ("authorships", [{"author": {"display_name": 123}}]),
        ("language", []),
        ("publication_date", "2026-02-30"),
        ("publication_date", "20261008"),
        ("publication_date", False),
        ("type", []),
        ("primary_topic", []),
        ("primary_topic", {"domain": {"id": "https://evil.test/domains/1"}}),
        ("primary_topic", {"domain": {"id": "https://openalex.org/domains/5"}}),
    ],
)
@respx.mock
async def test_openalex_invalid_metadata_is_502(client, field, value):
    respx.get(OPENALEX).mock(
        return_value=httpx.Response(200, json=openalex_page(results=[{**WORK, field: value}]))
    )
    response = await client.get(ENDPOINT, params={"q": "science", "source": "openalex"})
    assert response.status_code == 502
    assert response.json()["detail"]["provider"] == "openalex"
    assert "invalid catalog metadata" in response.text


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("id", "1342"),
        ("id", True),
        ("id", 0),
        ("title", None),
        ("authors", [{}]),
        ("authors", None),
        ("languages", "en"),
        ("subjects", [1]),
    ],
)
@respx.mock
async def test_gutendex_invalid_metadata_is_502(client, field, value):
    respx.get(GUTENDEX).mock(
        return_value=httpx.Response(200, json=gutendex_page(results=[{**BOOK, field: value}]))
    )
    response = await client.get(ENDPOINT, params={"q": "book"})
    assert response.status_code == 502
    assert response.json()["detail"]["provider"] == "gutendex"


@pytest.mark.parametrize("provider", ["gutendex", "openalex"])
@pytest.mark.parametrize("count", [-1, True, "1", None])
@respx.mock
async def test_bad_provider_count_is_never_coerced(client, provider, count):
    payload = gutendex_page(count) if provider == "gutendex" else openalex_page(count)
    respx.get(GUTENDEX if provider == "gutendex" else OPENALEX).mock(
        return_value=httpx.Response(200, json=payload)
    )
    response = await client.get(ENDPOINT, params={"q": "book", "source": provider})
    assert response.status_code == 502


@pytest.mark.parametrize("provider", ["gutendex", "openalex"])
@pytest.mark.parametrize("payload", [None, [], {}, {"results": {}}, {"count": 0, "results": [{}]}])
@respx.mock
async def test_malformed_provider_envelopes(client, provider, payload):
    respx.get(GUTENDEX if provider == "gutendex" else OPENALEX).mock(
        return_value=httpx.Response(200, json=payload)
    )
    response = await client.get(ENDPOINT, params={"q": "book", "source": provider})
    assert response.status_code == 502


@respx.mock
async def test_openalex_nullable_optional_metadata_and_display_name_fallback():
    work = {
        "id": "W12345",
        "display_name": "Untyped work",
        "language": None,
        "publication_date": None,
        "type": None,
        "primary_topic": None,
    }
    respx.get(OPENALEX).mock(return_value=httpx.Response(200, json=openalex_page(results=[work])))
    result = (await OpenAlexCatalogClient().search(query="work")).results[0]
    assert result.id == "W12345"
    assert result.title == "Untyped work"
    assert result.language == result.kind == ""
    assert result.authors == result.categories == []
    assert result.publication_date is None
    assert result.url == "https://openalex.org/W12345"


@respx.mock
async def test_direct_adapter_gutendex_and_id_mismatch():
    respx.get(f"{GUTENDEX}1342/").mock(return_value=httpx.Response(200, json={**BOOK, "id": 1}))
    with pytest.raises(CatalogProviderError, match="invalid catalog metadata"):
        await GutendexCatalogClient().search(gutenberg_id=1342)


@pytest.mark.parametrize("meta", [{"count": 1, "page": 2}, {"count": 1, "per_page": 100}])
@respx.mock
async def test_wrong_openalex_page_metadata_fails(client, meta):
    respx.get(OPENALEX).mock(
        return_value=httpx.Response(200, json={"meta": meta, "results": [WORK]})
    )
    response = await client.get(ENDPOINT, params={"q": "book", "source": "openalex"})
    assert response.status_code == 502


@respx.mock
async def test_openalex_last_basic_page_keeps_real_total_and_explicit_limit(client):
    route = respx.get(OPENALEX).mock(return_value=httpx.Response(200, json=openalex_page(20_000)))
    response = await client.get(ENDPOINT, params={"q": "book", "source": "openalex", "page": 312})
    assert response.status_code == 200
    assert route.calls.last.request.url.params["per_page"] == "32"
    assert response.json()["count"] == 20_000
    assert response.json()["next_page"] is None
    assert len(response.json()["warnings"]) == 1
    assert "9,984 accessible results" in response.json()["warnings"][0]
    assert "Narrow the query or filters" in response.json()["warnings"][0]
    response = await client.get(ENDPOINT, params={"q": "book", "source": "openalex", "page": 313})
    assert response.status_code == 422
    assert "Narrow the query" in response.text
    assert route.call_count == 1


@pytest.mark.parametrize("mode", ["count_exceeds_offset", "fallback_fails"])
@respx.mock
async def test_gutendex_missing_page_not_silently_treated_as_exhausted(client, mode):
    respx.get(GUTENDEX, params={"page": "3", "search": "science"}).mock(
        return_value=httpx.Response(404)
    )
    fallback = (
        httpx.Response(500)
        if mode == "fallback_fails"
        else httpx.Response(200, json=gutendex_page(100))
    )
    respx.get(GUTENDEX, params={"page": "1", "search": "science"}).mock(return_value=fallback)
    response = await client.get(ENDPOINT, params={"q": "science", "page": 3})
    assert response.status_code == 502


@respx.mock
async def test_invalid_second_provider_metadata_never_returns_partial_results(client):
    respx.get(GUTENDEX).mock(return_value=httpx.Response(200, json=gutendex_page()))
    payload = deepcopy(openalex_page())
    payload["results"].append({"id": "invalid"})
    respx.get(OPENALEX).mock(return_value=httpx.Response(200, json=payload))
    response = await client.get(
        ENDPOINT,
        params=[
            ("q", "science"),
            ("source", "gutendex"),
            ("source", "openalex"),
        ],
    )
    assert response.status_code == 502
    assert "results" not in response.json()


@pytest.mark.parametrize(
    ("url", "params"),
    [
        (GUTENDEX, {"q": "book"}),
        (GUTENDEX, {"q": "book", "page": 3}),
        (f"{GUTENDEX}1342/", {"gutenberg_id": 1342}),
        (OPENALEX, {"q": "work", "source": "openalex"}),
    ],
)
@respx.mock
async def test_literal_json_null_is_not_a_missing_book_or_exhausted_page(client, url, params):
    route = respx.get(url).mock(return_value=httpx.Response(200, text="null"))
    response = await client.get(ENDPOINT, params=params)
    assert response.status_code == 502
    assert "invalid catalog metadata" in response.text
    assert route.call_count == 1


@pytest.mark.parametrize("provider", ["gutendex", "openalex"])
@pytest.mark.parametrize(
    ("page", "count", "result_count"),
    [(1, 1, 0), (1, 0, 1), (2, 1, 1), (2, 33, 2), (1, 100, 33)],
)
@respx.mock
async def test_inconsistent_page_counts_are_provider_errors(
    client, provider, page, count, result_count
):
    payload = (
        gutendex_page(count, [BOOK] * result_count)
        if provider == "gutendex"
        else openalex_page(count, [WORK] * result_count)
    )
    respx.get(GUTENDEX if provider == "gutendex" else OPENALEX).mock(
        return_value=httpx.Response(200, json=payload)
    )
    response = await client.get(ENDPOINT, params={"q": "book", "source": provider, "page": page})
    assert response.status_code == 502
    assert response.json()["detail"]["provider"] == provider


@respx.mock
async def test_empty_api_key_stays_keyless(monkeypatch):
    monkeypatch.setattr(settings, "openalex_api_key", SecretStr(""))
    route = respx.get(OPENALEX).mock(return_value=httpx.Response(200, json=openalex_page()))
    await OpenAlexCatalogClient().search(query="work")
    assert "authorization" not in route.calls.last.request.headers


@respx.mock
async def test_empty_service_filter_selections_default_to_unrestricted_gutendex():
    route = respx.get(GUTENDEX).mock(return_value=httpx.Response(200, json=gutendex_page()))
    response = await catalog.CatalogSearchService().search(
        q="Austen", source=[], date=[], category=[], kind=[]
    )
    assert response.count == 1
    assert dict(route.calls.last.request.url.params) == {"search": "Austen", "page": "1"}


@respx.mock
async def test_gutendex_next_page_uses_total_without_following_upstream_links(client):
    route = respx.get(GUTENDEX).mock(return_value=httpx.Response(200, json=gutendex_page(70)))
    response = await client.get(ENDPOINT, params={"q": "book", "page": 2})
    assert response.status_code == 200
    assert response.json()["next_page"] == 3
    assert route.call_count == 1


@pytest.mark.parametrize("today", ["2024-02-29", "2025-01-01", "2026-10-08"])
def test_past_year_has_exactly_365_inclusive_utc_dates(today):
    end_date = datetime.fromisoformat(today).date()
    start, end = catalog._date_range({"today", "7days", "30days", "year"}, end_date)
    assert end == today
    assert (end_date - datetime.fromisoformat(start).date()).days + 1 == 365


@respx.mock
async def test_openalex_other_is_exact_type_not_a_catch_all_filter(client):
    route = respx.get(OPENALEX).mock(
        return_value=httpx.Response(200, json=openalex_page(results=[{**WORK, "type": "other"}]))
    )
    response = await client.get(
        ENDPOINT, params={"q": "work", "source": "openalex", "kind": "other"}
    )
    assert response.status_code == 200
    assert route.calls.last.request.url.params["filter"] == "type:other"
    assert response.json()["results"][0]["kind"] == "other"
    assert response.json()["warnings"] == []


@pytest.mark.parametrize(
    "subject",
    [
        "Fiction&ids=999&page=313#fragment?source=openalex",
        "Fiction%26ids%3D999&topic=Science",
        "Fiction\r\nAuthorization: Bearer injected",
    ],
)
@respx.mock
async def test_gutendex_subject_cannot_inject_query_params_headers_or_fragments(client, subject):
    route = respx.get(GUTENDEX).mock(return_value=httpx.Response(200, json=gutendex_page()))
    response = await client.get(ENDPOINT, params={"q": "Austen", "category": f"subject:{subject}"})
    assert response.status_code == 200
    request = route.calls.last.request
    assert dict(request.url.params) == {"search": "Austen", "page": "1", "topic": subject}
    assert request.url.params.get_list("topic") == [subject]
    assert request.url.host == "gutendex.test"
    assert request.url.fragment == ""
    assert "authorization" not in request.headers
    assert route.call_count == 1


@respx.mock
async def test_null_openalex_title_uses_documented_display_name_without_dropping_records(client):
    work = {
        **WORK,
        "id": "https://openalex.org/W67890",
        "title": None,
        "display_name": "Documented title alias",
    }
    respx.get(OPENALEX).mock(return_value=httpx.Response(200, json=openalex_page(2, [WORK, work])))
    response = await client.get(ENDPOINT, params={"q": "work", "source": "openalex"})
    assert response.status_code == 200
    assert response.json()["count"] == 2
    assert [record["title"] for record in response.json()["results"]] == [
        "A scientific work",
        "Documented title alias",
    ]
    assert response.json()["warnings"] == []


@pytest.mark.parametrize("display_name", [None, [], 123, ""])
@respx.mock
async def test_null_openalex_title_with_invalid_alias_still_fails_page(client, display_name):
    work = {**WORK, "title": None, "display_name": display_name}
    respx.get(OPENALEX).mock(return_value=httpx.Response(200, json=openalex_page(results=[work])))
    response = await client.get(ENDPOINT, params={"q": "work", "source": "openalex"})
    assert response.status_code == 502
    assert "invalid catalog metadata" in response.text


@pytest.mark.parametrize("title", ["A scientific work", [], ""])
@respx.mock
async def test_present_non_null_openalex_title_is_not_overridden(client, title):
    work = {**WORK, "title": title, "display_name": "Different alias"}
    respx.get(OPENALEX).mock(return_value=httpx.Response(200, json=openalex_page(results=[work])))
    response = await client.get(ENDPOINT, params={"q": "work", "source": "openalex"})
    if isinstance(title, str) and title:
        assert response.status_code == 200
        assert response.json()["results"][0]["title"] == title
    else:
        assert response.status_code == 502


@pytest.mark.parametrize(
    ("page", "count", "expected_next", "has_warning"),
    [(311, 20_000, 312, False), (312, 9_984, None, False), (312, 9_985, None, True)],
)
@respx.mock
async def test_openalex_adapter_caps_next_page_only_at_supported_boundary(
    page, count, expected_next, has_warning
):
    respx.get(OPENALEX).mock(return_value=httpx.Response(200, json=openalex_page(count)))
    result = await OpenAlexCatalogClient().search(query="work", page=page)
    assert result.count == count
    assert result.next_page == expected_next
    assert bool(result.warnings) == has_warning


@pytest.mark.parametrize(
    ("count_g", "count_o"),
    [(20_000, 20_000), (20_000, 1), (1, 20_000), (9_984, 9_984), (0, 0), (20_000, 9_984)],
)
@respx.mock
async def test_combined_catalog_last_page_is_capped_without_changing_counts(
    client, count_g, count_o
):
    offset = 311 * 32
    gutendex = respx.get(GUTENDEX).mock(
        return_value=httpx.Response(
            200, json=gutendex_page(count_g, [BOOK] if count_g > offset else [])
        )
    )
    openalex = respx.get(OPENALEX).mock(
        return_value=httpx.Response(
            200, json=openalex_page(count_o, [WORK] if count_o > offset else [])
        )
    )
    response = await client.get(
        ENDPOINT,
        params=[("q", "work"), ("source", "openalex"), ("source", "gutendex"), ("page", "312")],
    )
    assert response.status_code == 200
    body = response.json()
    assert body["count"] == count_g + count_o
    assert body["next_page"] is None
    assert body["previous_page"] == 311
    assert len(body["warnings"]) == int(count_o > 9_984) + int(count_g > 9_984)
    if count_g > 9_984:
        assert "Narrow the query or filters" in body["warnings"][-1]
        assert "source=gutendex" in body["warnings"][-1]
        assert "More Gutendex matches remain" in body["warnings"][-1]
    if count_o > 9_984:
        assert "More OpenAlex matches remain" in body["warnings"][0]
    assert gutendex.calls.last.request.url.params["page"] == "312"
    assert openalex.calls.last.request.url.params["page"] == "312"
    assert gutendex.call_count == openalex.call_count == 1


@respx.mock
async def test_gutendex_only_can_continue_past_openalex_limit_without_warning(client):
    route = respx.get(GUTENDEX).mock(return_value=httpx.Response(200, json=gutendex_page(20_000)))
    for page in (312, 313):
        response = await client.get(
            ENDPOINT, params={"q": "book", "source": "gutendex", "page": page}
        )
        assert response.status_code == 200
        assert response.json()["count"] == 20_000
        assert response.json()["next_page"] == page + 1
        assert response.json()["warnings"] == []
    assert route.call_count == 2


def test_catalog_warnings_default_is_independent_per_response():
    from metabook_py.models.catalog import CatalogSearchResponse

    first = CatalogSearchResponse(count=0, page=1)
    first.warnings.append("Limit warning")
    assert CatalogSearchResponse(count=0, page=1).warnings == []
