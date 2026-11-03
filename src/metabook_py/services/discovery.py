"""
Gutendex discovery service.

Responsibilities
----------------
- Map caller-supplied search params (title / isbn / gutenberg_id) to Gutendex
  query parameters.
- Parse the Gutendex JSON response into typed Pydantic models.
- Raise domain exceptions (BookNotFoundError, AmbiguousBookError,
  UnsupportedFormatError) so callers never deal with raw HTTP status codes.
- Return a (BookInfo, download_url, is_html) triple for the resolved book.
"""

import asyncio

import httpx

from metabook_py.core.config import settings
from metabook_py.core.exceptions import (
    AmbiguousBookError,
    BookNotFoundError,
    GutendexUnavailableError,
    UnsupportedFormatError,
)
from metabook_py.models.book import AuthorInfo, BookInfo, BookMatch, BookSearchResponse

# Priority order for format selection.  The first key found in the Gutendex
# "formats" dict is used; is_html is True only for HTML formats.
_FORMAT_PRIORITY: list[tuple[str, bool]] = [
    ("text/plain; charset=utf-8", False),
    ("text/plain; charset=us-ascii", False),
    ("text/plain", False),
    ("text/html; charset=utf-8", True),
    ("text/html", True),
]


def _pick_format(formats: dict[str, str], gutenberg_id: int) -> tuple[str, bool]:
    """Return (url, is_html) for the best available text format."""
    for mime, is_html in _FORMAT_PRIORITY:
        if mime in formats:
            return formats[mime], is_html

    # Fallback: any key that contains 'text'
    for key, url in formats.items():
        if "text" in key and "zip" not in key:
            return url, "html" in key

    raise UnsupportedFormatError(
        gutenberg_id=gutenberg_id,
        available_formats=list(formats.keys()),
    )


def _parse_book(data: dict) -> BookInfo:
    return BookInfo(
        gutenberg_id=data["id"],
        title=data["title"],
        authors=[
            AuthorInfo(
                name=a["name"],
                birth_year=a.get("birth_year"),
                death_year=a.get("death_year"),
            )
            for a in data.get("authors", [])
        ],
        language=",".join(data.get("languages", ["en"])),
        subjects=data.get("subjects", []),
    )


class GutendexClient:
    """
    Thin async wrapper around the Gutendex REST API.

    Uses a per-instance asyncio.Semaphore so multiple concurrent requests
    from the same process do not overwhelm Gutendex (max 1 in-flight at a time).
    """

    def __init__(self) -> None:
        self._semaphore = asyncio.Semaphore(1)

    async def _request_books(self, params: dict[str, str], *, book_id: int | None = None) -> dict:
        async with self._semaphore:
            try:
                async with httpx.AsyncClient(timeout=30.0) as client:
                    path = f"books/{book_id}/" if book_id is not None else "books/"
                    resp = await client.get(
                        f"{settings.gutendex_base_url.rstrip('/')}/{path}", params=params
                    )
                    if book_id is not None and resp.status_code == 404:
                        return {"count": 0, "results": []}
                    resp.raise_for_status()
            except httpx.TimeoutException as exc:
                raise GutendexUnavailableError(
                    str(exc) or "request timed out", timed_out=True
                ) from exc
            except httpx.HTTPError as exc:
                raise GutendexUnavailableError(str(exc)) from exc
        try:
            payload = resp.json()
            if book_id is not None:
                if not isinstance(payload, dict) or payload.get("id") != book_id:
                    raise ValueError("Expected the requested book")
                return {"count": 1, "results": [payload]}
            if not isinstance(payload, dict) or not isinstance(payload.get("results"), list):
                raise ValueError("Expected a book list")
            return payload
        except ValueError as exc:
            raise GutendexUnavailableError("Gutendex returned an invalid book list") from exc

    async def list_books(
        self,
        *,
        query: str | None = None,
        isbn: str | None = None,
        gutenberg_id: int | None = None,
        language: str | None = None,
        page: int = 1,
    ) -> BookSearchResponse:
        """Browse metadata only; Gutendex provides up to 32 results per page.

        ISBN is a best-effort keyword search: Gutendex has no ISBN index.
        Pagination always uses our configured origin, never an upstream link.
        """
        params = {"page": str(page)}
        if language:
            params["languages"] = language
        if gutenberg_id is not None:
            params["ids"] = str(gutenberg_id)
        elif isbn:
            params["search"] = isbn
        elif query:
            params["search"] = query
        payload = await self._request_books(
            {} if gutenberg_id is not None else params, book_id=gutenberg_id
        )
        if gutenberg_id is not None:
            if language:
                payload["results"] = [
                    book
                    for book in payload["results"]
                    if set(language.split(",")) & set(book.get("languages", []))
                ]
                payload["count"] = len(payload["results"])
            if page > 1:
                payload["results"] = []
        try:
            return BookSearchResponse(
                count=payload["count"],
                page=page,
                next_page=page + 1 if payload.get("next") else None,
                previous_page=page - 1 if page > 1 and payload.get("previous") else None,
                results=[
                    BookMatch(
                        gutenberg_id=book["id"],
                        title=book["title"],
                        authors=[author["name"] for author in book.get("authors", [])],
                        language=",".join(book.get("languages", [])),
                    )
                    for book in payload["results"]
                ],
            )
        except (KeyError, TypeError, ValueError) as exc:
            raise GutendexUnavailableError("Gutendex returned invalid book metadata") from exc

    async def search(
        self,
        *,
        title: str | None = None,
        isbn: str | None = None,
        gutenberg_id: int | None = None,
        language: str = "en",
    ) -> tuple[BookInfo, str, bool]:
        """
        Search Gutendex for a book.

        Returns
        -------
        (BookInfo, download_url, is_html)

        Raises
        ------
        BookNotFoundError         — zero results
        AmbiguousBookError        — multiple results when no gutenberg_id given
        UnsupportedFormatError    — book found but no text format available
        GutendexUnavailableError  — Gutendex timed out, refused the connection,
                                    or returned a non-2xx response
        """
        params: dict[str, str] = {"languages": language} if language else {}

        if gutenberg_id is not None:
            params["ids"] = str(gutenberg_id)
        elif isbn:
            # Gutendex has no dedicated ISBN filter; search= is best-effort
            params["search"] = isbn
        elif title:
            params["search"] = title

        payload = await self._request_books(
            {} if gutenberg_id is not None else params, book_id=gutenberg_id
        )
        results: list[dict] = payload.get("results", [])
        query = {"title": title, "isbn": isbn, "gutenberg_id": gutenberg_id}

        if not results:
            raise BookNotFoundError(query=query)

        # Multiple results — ask caller to disambiguate (unless they gave an id)
        if len(results) > 1 and gutenberg_id is None:
            matches = [
                BookMatch(
                    gutenberg_id=r["id"],
                    title=r["title"],
                    authors=[a["name"] for a in r.get("authors", [])],
                    language=",".join(r.get("languages", ["?"])),
                )
                for r in results[: settings.max_disambiguations]
            ]
            raise AmbiguousBookError(matches=matches)

        book_data = results[0]
        book_info = _parse_book(book_data)
        url, is_html = _pick_format(book_data.get("formats", {}), book_info.gutenberg_id)

        return book_info, url, is_html
