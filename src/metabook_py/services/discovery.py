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
    UnsupportedFormatError,
)
from metabook_py.models.book import AuthorInfo, BookInfo, BookMatch

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
        BookNotFoundError       — zero results
        AmbiguousBookError      — multiple results when no gutenberg_id given
        UnsupportedFormatError  — book found but no text format available
        """
        params: dict[str, str] = {"languages": language}

        if gutenberg_id is not None:
            params["ids"] = str(gutenberg_id)
        elif isbn:
            # Gutendex has no dedicated ISBN filter; search= is best-effort
            params["search"] = isbn
        elif title:
            params["search"] = title

        async with self._semaphore:
            async with httpx.AsyncClient(timeout=30.0) as client:
                resp = await client.get(
                    f"{settings.gutendex_base_url}/books/",
                    params=params,
                )
                resp.raise_for_status()

        payload = resp.json()
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
