"""Best-effort catalog enrichment. Source structure remains authoritative."""

import asyncio
import re
import time
import unicodedata
from datetime import UTC, datetime

import httpx

from metabook_py.core.cache import AsyncInMemoryCache
from metabook_py.core.config import settings
from metabook_py.models.book import OpenLibraryMetadata, UploadedBookInfo

_cache = AsyncInMemoryCache()
_lock = asyncio.Lock()
_next_request = 0.0


def _strings(value: object) -> list[str]:
    return [x for x in value if isinstance(x, str)] if isinstance(value, list) else []


def _id(value: object, kind: str) -> str | None:
    if isinstance(value, str):
        key = value.rsplit("/", 1)[-1]
        if re.fullmatch(r"OL[0-9]+" + kind, key):
            return key
    return None


def _normalized(value: str) -> str:
    return " ".join(sorted(re.findall(r"\w+", unicodedata.normalize("NFKC", value).casefold())))


def _isbn(value: str | None) -> str | None:
    value = re.sub(r"[\s-]", "", value or "").upper()
    if (
        re.fullmatch(r"\d{13}", value)
        and sum(int(c) * (1 if i % 2 == 0 else 3) for i, c in enumerate(value)) % 10 == 0
    ):
        return value
    if (
        re.fullmatch(r"\d{9}[\dX]", value)
        and sum((10 if c == "X" else int(c)) * (10 - i) for i, c in enumerate(value)) % 11 == 0
    ):
        return value
    return None


async def _get(params: dict) -> dict:
    global _next_request
    key = str(sorted(params.items()))
    cached = await _cache.get(key)
    if cached is not None:
        return cached
    async with _lock:
        cached = await _cache.get(key)
        if cached is not None:
            return cached
        await asyncio.sleep(max(0, _next_request - time.monotonic()))
        _next_request = time.monotonic() + 1.0
        endpoint = "api/books" if "bibkeys" in params else "search.json"
        async with httpx.AsyncClient(timeout=settings.open_library_timeout) as client:
            response = await client.get(
                f"https://openlibrary.org/{endpoint}",
                params=params,
                headers={"User-Agent": settings.open_library_user_agent},
            )
            response.raise_for_status()
            data = response.json()
        if not isinstance(data, dict):
            raise ValueError("Invalid catalog response")
        if _cache.size >= 512:
            _cache.clear()
        await _cache.set(key, data)
        return data


async def _lookup(book: UploadedBookInfo) -> UploadedBookInfo:
    isbn = _isbn(book.isbn)
    fields = {}
    if isbn:
        key = f"ISBN:{isbn}"
        data = (await _get({"bibkeys": key, "format": "json", "jscmd": "data"})).get(key)
        if not isinstance(data, dict):
            return book
        identifiers = data.get("identifiers", {})
        editions = _strings(identifiers.get("openlibrary"))
        edition = _id(editions[0], "M") if editions else None
        if not edition:
            return book
        cover = data.get("cover", {}).get("large", "")
        cover_match = re.search(r"/b/id/(\d+)-", cover) if isinstance(cover, str) else None
        subjects = [
            x["name"]
            for x in data.get("subjects", [])
            if isinstance(x, dict) and isinstance(x.get("name"), str)
        ]
        classifications = data.get("classifications", {})
        extra = OpenLibraryMetadata(
            match_method="isbn",
            edition_id=edition,
            cover_id=int(cover_match[1]) if cover_match else None,
            subjects=subjects,
            dewey_decimal=_strings(classifications.get("dewey_decimal_class")),
            lc_classifications=_strings(classifications.get("lc_classifications")),
            fetched_at=datetime.now(UTC).isoformat(),
        )
        publishers = data.get("publishers", [])
        publisher = (
            publishers[0].get("name") if publishers and isinstance(publishers[0], dict) else None
        )
        pages = data.get("number_of_pages")
        if not book.publisher and isinstance(publisher, str):
            fields["publisher"] = publisher
        if not book.date and isinstance(data.get("publish_date"), str):
            fields["date"] = data["publish_date"]
        if not book.number_of_pages and isinstance(pages, int) and pages > 0:
            fields["number_of_pages"] = pages
    else:
        if not book.authors or not book.title.strip():
            return book
        data = await _get(
            {
                "title": book.title,
                "author": book.authors[0].name,
                "limit": 10,
                "fields": "key,title,author_name,cover_i,subject",
            }
        )
        # Never accept a truncated candidate set or ambiguous exact matches.
        if data.get("numFound", 0) > 10:
            return book
        authors = {_normalized(a.name) for a in book.authors}
        matches = [
            d
            for d in data.get("docs", [])
            if isinstance(d, dict)
            and isinstance(d.get("title"), str)
            and _normalized(d["title"]) == _normalized(book.title)
            and authors.intersection(_normalized(a) for a in _strings(d.get("author_name")))
        ]
        if len(matches) != 1:
            return book
        match = matches[0]
        work = _id(match.get("key"), "W")
        if not work:
            return book
        cover = match.get("cover_i")
        extra = OpenLibraryMetadata(
            match_method="title_author",
            work_id=work,
            cover_id=cover if isinstance(cover, int) and cover > 0 else None,
            subjects=_strings(match.get("subject")),
            fetched_at=datetime.now(UTC).isoformat(),
        )
    return book.model_copy(update={**fields, "open_library": extra})


async def enrich_uploaded_book(book: UploadedBookInfo) -> UploadedBookInfo:
    """Bound network/queue time; a failed lookup must never discard an upload."""
    if not settings.open_library_enabled:
        return book
    try:
        async with asyncio.timeout(settings.open_library_timeout):
            return await _lookup(book)
    except (httpx.HTTPError, TimeoutError, ValueError, TypeError, KeyError, AttributeError):
        return book
