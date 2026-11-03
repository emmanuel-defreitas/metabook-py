"""Validate all selected providers before network I/O, then aggregate metadata pages."""

from datetime import UTC, date, datetime, timedelta

from metabook_py.core.exceptions import CatalogValidationError
from metabook_py.models.catalog import CatalogSearchResponse
from metabook_py.services.catalog_providers import (
    OPENALEX_DOMAINS,
    OPENALEX_MAX_PAGE,
    PAGE_SIZE,
    GutendexCatalogClient,
    OpenAlexCatalogClient,
)

_PROVIDER_ORDER = ("gutendex", "openalex")
_DATE_KEYS = {"today", "7days", "30days", "year"}
_KIND_KEYS = {"book", "academic", "other", "canon", "corpus", "collection"}
_ACADEMIC_TYPES = ("article", "preprint", "review", "dissertation", "report", "conference-paper")


def _date_range(keys: set[str], today: date) -> tuple[str | None, str | None]:
    if not keys:
        return None, None
    starts = {
        "today": today,
        "7days": today - timedelta(days=6),
        "30days": today - timedelta(days=29),
        "year": today - timedelta(days=364),
    }
    # These inclusive ranges all end today; their OR is the widest selected range.
    return min(starts[key] for key in keys).isoformat(), today.isoformat()


class CatalogSearchService:
    def __init__(self) -> None:
        self.gutendex = GutendexCatalogClient()
        self.openalex = OpenAlexCatalogClient()

    async def search(
        self,
        *,
        q: str | None = None,
        isbn: str | None = None,
        gutenberg_id: int | None = None,
        page: int = 1,
        date: list[str] | None = None,
        category: list[str] | None = None,
        kind: list[str] | None = None,
        source: list[str] | None = None,
    ) -> CatalogSearchResponse:
        q = (q or "").strip() or None
        isbn = (isbn or "").strip() or None
        if not any((q, isbn, gutenberg_id)):
            raise CatalogValidationError("Provide a query (q), ISBN, or Gutenberg ID.")
        if page < 1 or (gutenberg_id is not None and gutenberg_id < 1):
            raise CatalogValidationError("page and gutenberg_id must be positive integers.")

        sources = set(source or ["gutendex"])
        dates = set(date or [])
        categories = set(category or [])
        kinds = set(kind or [])
        if "upload" in sources:
            raise CatalogValidationError(
                "source upload is saved-only, not a live catalog provider."
            )
        if "kindle" in sources:
            raise CatalogValidationError(
                "source kindle is not connected to a live catalog provider."
            )
        if unknown := sources - set(_PROVIDER_ORDER):
            raise CatalogValidationError(
                f"Unknown source filter key(s): {', '.join(sorted(unknown))}."
            )
        if unknown := dates - _DATE_KEYS:
            raise CatalogValidationError(
                f"Unknown date filter key(s): {', '.join(sorted(unknown))}."
            )
        if unknown := kinds - _KIND_KEYS:
            raise CatalogValidationError(
                f"Unknown kind filter key(s): {', '.join(sorted(unknown))}."
            )
        subjects = []
        domains = []
        for key in sorted(categories):
            if key in OPENALEX_DOMAINS:
                domains.append(key.removeprefix("domain:"))
            elif key.startswith("subject:") and key[8:].strip() and key[8:] == key[8:].strip():
                subjects.append(key[8:])
            else:
                raise CatalogValidationError(f"Unknown or invalid category filter key: {key!r}.")
        if saved_only := kinds & {"canon", "corpus", "collection"}:
            raise CatalogValidationError(
                f"Kind filter(s) {', '.join(sorted(saved_only))} are schema-specific saved-only "
                "filters, unsupported by live catalog providers."
            )

        # Complete capability validation for *every* source before the first request.
        if "gutendex" in sources:
            if dates:
                raise CatalogValidationError(
                    "Gutendex has no publication dates; date filters are unsupported. "
                    "Select only OpenAlex to filter by publication date."
                )
            if domains:
                raise CatalogValidationError(
                    "Gutendex does not support OpenAlex domain categories; use subject:<exact subject>."
                )
            if len(subjects) > 1:
                raise CatalogValidationError(
                    "Gutendex supports one topic only; OR across multiple subject categories "
                    "is unsupported. Select a single subject."
                )
            if unsupported := kinds - {"book"}:
                raise CatalogValidationError(
                    "Gutendex supports unrestricted or book-only kind filters; unsupported "
                    f"kind(s): {', '.join(sorted(unsupported))}."
                )
        if "openalex" in sources:
            if isbn or gutenberg_id is not None:
                raise CatalogValidationError(
                    "OpenAlex catalog search does not support ISBN or Gutenberg ID lookup; "
                    "use q with OpenAlex, or select Gutendex for these identifiers."
                )
            if subjects:
                raise CatalogValidationError(
                    "OpenAlex does not support Gutendex subject categories; use domain:1..4."
                )
            if page > OPENALEX_MAX_PAGE:
                raise CatalogValidationError(
                    f"OpenAlex basic paging with per_page={PAGE_SIZE} supports pages 1–"
                    f"{OPENALEX_MAX_PAGE} (10,000-result limit). Narrow the query or filters."
                )

        types: tuple[str, ...] = ("book",) if "book" in kinds else ()
        if "academic" in kinds:
            types += _ACADEMIC_TYPES
        if "other" in kinds:
            types += ("other",)
        start, end = _date_range(dates, datetime.now(UTC).date())
        pages = []
        for provider in _PROVIDER_ORDER:
            if provider not in sources:
                continue
            if provider == "gutendex":
                result = await self.gutendex.search(
                    query=q,
                    isbn=isbn,
                    gutenberg_id=gutenberg_id,
                    page=page,
                    subject=subjects[0] if subjects else None,
                )
            else:
                result = await self.openalex.search(
                    query=q or "",
                    page=page,
                    domains=tuple(domains),
                    types=types,
                    date_start=start,
                    date_end=end,
                )
            pages.append(result)
        # No partial response: exceptions from any provider abort the entire search.
        warnings = [warning for result in pages for warning in result.warnings]
        next_page = page + 1 if any(result.next_page for result in pages) else None
        if "openalex" in sources and page == OPENALEX_MAX_PAGE and next_page is not None:
            next_page = None
            warnings.append(
                f"Combined catalog paging stops at page {OPENALEX_MAX_PAGE} because OpenAlex "
                "is selected. More Gutendex matches remain in the reported count. Narrow the "
                "query or filters, or switch to source=gutendex to continue Gutendex-only paging."
            )
        return CatalogSearchResponse(
            count=sum(result.count for result in pages),
            page=page,
            next_page=next_page,
            previous_page=page - 1 if page > 1 else None,
            results=[book for result in pages for book in result.results],
            warnings=warnings,
        )
