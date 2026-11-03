"""Metadata-only Gutendex and OpenAlex adapters. No scanning, persistence, or URL following."""

import re
from datetime import date

import httpx
from pydantic import AliasChoices, BaseModel, ConfigDict, Field, field_validator, model_validator

from metabook_py.core.config import settings
from metabook_py.core.exceptions import CatalogProviderError, CatalogValidationError
from metabook_py.models.catalog import CatalogResult, CatalogSearchResponse, CatalogSource

PAGE_SIZE = 32
OPENALEX_MAX_PAGE = 10_000 // PAGE_SIZE
_NOT_FOUND = object()
# Verified against https://api.openalex.org/domains (not field IDs).
OPENALEX_DOMAINS = {
    "domain:1": "Life Sciences",
    "domain:2": "Social Sciences",
    "domain:3": "Physical Sciences",
    "domain:4": "Health Sciences",
}


class _Metadata(BaseModel):
    model_config = ConfigDict(strict=True)


class _GutendexAuthor(_Metadata):
    name: str


class _GutendexBook(_Metadata):
    id: int = Field(gt=0)
    title: str = Field(min_length=1)
    authors: list[_GutendexAuthor] = Field(default_factory=list)
    languages: list[str] = Field(default_factory=list)
    subjects: list[str] = Field(default_factory=list)

    def result(self) -> CatalogResult:
        return CatalogResult(
            source="gutendex",
            id=str(self.id),
            gutenberg_id=self.id,
            title=self.title,
            authors=[author.name for author in self.authors],
            language=",".join(self.languages),
            kind="book",
            categories=[f"subject:{subject}" for subject in self.subjects],
            url=f"https://www.gutenberg.org/ebooks/{self.id}",
        )


class _GutendexPage(_Metadata):
    count: int = Field(ge=0)
    next: str | None = None
    previous: str | None = None
    results: list[_GutendexBook] = Field(max_length=PAGE_SIZE)


class _OpenAlexAuthor(_Metadata):
    display_name: str


class _OpenAlexAuthorship(_Metadata):
    author: _OpenAlexAuthor


class _OpenAlexDomain(_Metadata):
    id: str = Field(pattern=r"^https://openalex\.org/domains/[1-4]$")


class _OpenAlexTopic(_Metadata):
    domain: _OpenAlexDomain | None = None


class _OpenAlexWork(_Metadata):
    id: str
    title: str = Field(validation_alias=AliasChoices("title", "display_name"), min_length=1)
    authorships: list[_OpenAlexAuthorship] = Field(default_factory=list)
    language: str | None = None
    publication_date: str | None = None
    type: str | None = None
    primary_topic: _OpenAlexTopic | None = None

    @model_validator(mode="before")
    @classmethod
    def title_from_display_name(cls, value: object) -> object:
        # OpenAlex documents display_name as the same work title, not a separate label.
        if isinstance(value, dict) and value.get("title") is None and "display_name" in value:
            return {**value, "title": value["display_name"]}
        return value

    @field_validator("id")
    @classmethod
    def valid_work_id(cls, value: str) -> str:
        match = re.fullmatch(r"(?:https://openalex\.org/)?(W[1-9][0-9]*)", value)
        if match is None:
            raise ValueError("Expected an OpenAlex W identifier")
        return match[1]

    @field_validator("publication_date")
    @classmethod
    def valid_publication_date(cls, value: str | None) -> str | None:
        if value is not None:
            if not re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}", value):
                raise ValueError("Expected an ISO publication date")
            date.fromisoformat(value)
        return value

    def result(self) -> CatalogResult:
        domain = self.primary_topic.domain if self.primary_topic else None
        return CatalogResult(
            source="openalex",
            id=self.id,
            title=self.title,
            authors=[authorship.author.display_name for authorship in self.authorships],
            language=self.language or "",
            publication_date=self.publication_date,
            kind=self.type or "",
            categories=[f"domain:{domain.id.rsplit('/', 1)[1]}"] if domain else [],
            # Do not expose primary_location, DOI links, or arbitrary upstream URLs as actions.
            url=f"https://openalex.org/{self.id}",
        )


class _OpenAlexMeta(_Metadata):
    count: int = Field(ge=0)
    page: int | None = None
    per_page: int | None = None


class _OpenAlexPage(_Metadata):
    meta: _OpenAlexMeta
    results: list[_OpenAlexWork] = Field(max_length=PAGE_SIZE)


async def _request_json(
    provider: CatalogSource,
    url: str,
    params: dict[str, str],
    *,
    headers: dict[str, str] | None = None,
    allow_not_found: bool = False,
) -> object:
    try:
        async with httpx.AsyncClient(timeout=30.0) as client:
            response = await client.get(url, params=params, headers=headers)
            if response.status_code == 404 and allow_not_found:
                return _NOT_FOUND
            if response.status_code == 429:
                advice = "Retry later with backoff."
                if provider == "openalex":
                    advice += (
                        " A free server-side OpenAlex API key raises the keyless daily budget."
                    )
                raise CatalogProviderError(
                    provider, f"{provider} rate limit exceeded. {advice}", status_code=429
                )
            if response.status_code in (401, 403) and provider == "openalex":
                raise CatalogProviderError(
                    provider, "OpenAlex authentication failed; check the server's OPENALEX_API_KEY."
                )
            if not response.is_success:
                raise CatalogProviderError(
                    provider, f"{provider} returned HTTP {response.status_code}; retry later."
                )
    except httpx.TimeoutException as exc:
        raise CatalogProviderError(
            provider, f"{provider} request timed out; retry later.", status_code=504
        ) from exc
    except httpx.HTTPError as exc:
        # httpx exception text can contain request URLs/credentials; don't echo it.
        raise CatalogProviderError(provider, f"Could not reach {provider}; retry later.") from exc
    try:
        return response.json()
    except ValueError as exc:
        raise CatalogProviderError(provider, f"{provider} returned invalid JSON.") from exc


def _invalid_metadata(provider: CatalogSource) -> CatalogProviderError:
    return CatalogProviderError(provider, f"{provider} returned invalid catalog metadata.")


def _validate_page_count(count: int, result_count: int, page: int) -> None:
    remaining = max(count - (page - 1) * PAGE_SIZE, 0)
    if result_count > remaining or (result_count == 0 and remaining > 0):
        raise ValueError("Results do not match count and page")


class GutendexCatalogClient:
    async def search(
        self,
        *,
        query: str | None = None,
        isbn: str | None = None,
        gutenberg_id: int | None = None,
        page: int = 1,
        subject: str | None = None,
    ) -> CatalogSearchResponse:
        base_url = settings.gutendex_base_url.rstrip("/")
        if gutenberg_id is not None and subject is None:
            payload = await _request_json(
                "gutendex", f"{base_url}/books/{gutenberg_id}/", {}, allow_not_found=True
            )
            try:
                book = _GutendexBook.model_validate(payload) if payload is not _NOT_FOUND else None
                if book and book.id != gutenberg_id:
                    raise ValueError("Wrong requested book")
                return CatalogSearchResponse(
                    count=1 if book else 0,
                    page=page,
                    previous_page=page - 1 if page > 1 else None,
                    results=[book.result()] if book and page == 1 else [],
                )
            except ValueError as exc:
                raise _invalid_metadata("gutendex") from exc

        params = {"page": str(page)}
        if gutenberg_id is not None:
            params["ids"] = str(gutenberg_id)
        elif search := isbn or query:
            params["search"] = search
        if subject is not None:
            params["topic"] = subject
        payload = await _request_json(
            "gutendex", f"{base_url}/books/", params, allow_not_found=page > 1
        )
        exhausted = payload is _NOT_FOUND
        if exhausted:
            # Gutendex returns 404 past its last page. Recover the *real* total,
            # rather than reporting zero and corrupting a multi-provider count.
            payload = await _request_json("gutendex", f"{base_url}/books/", {**params, "page": "1"})
        try:
            metadata = _GutendexPage.model_validate(payload)
            _validate_page_count(metadata.count, len(metadata.results), 1 if exhausted else page)
            if exhausted and metadata.count > (page - 1) * PAGE_SIZE:
                raise ValueError("Unexpected missing page")
            return CatalogSearchResponse(
                count=metadata.count,
                page=page,
                next_page=page + 1 if metadata.count > page * PAGE_SIZE else None,
                previous_page=page - 1 if page > 1 else None,
                results=[] if exhausted else [book.result() for book in metadata.results],
            )
        except ValueError as exc:
            raise _invalid_metadata("gutendex") from exc


class OpenAlexCatalogClient:
    async def search(
        self,
        *,
        query: str,
        page: int = 1,
        domains: tuple[str, ...] = (),
        types: tuple[str, ...] = (),
        date_start: str | None = None,
        date_end: str | None = None,
    ) -> CatalogSearchResponse:
        if not 1 <= page <= OPENALEX_MAX_PAGE:
            raise CatalogValidationError(
                f"OpenAlex basic paging with per_page={PAGE_SIZE} supports pages 1–"
                f"{OPENALEX_MAX_PAGE} (10,000-result limit). Narrow the query or filters."
            )
        params = {"search": query, "page": str(page), "per_page": str(PAGE_SIZE)}
        filters = []
        if domains:
            filters.append(f"primary_topic.domain.id:{'|'.join(domains)}")
        if types:
            filters.append(f"type:{'|'.join(types)}")
        if date_start:
            filters.append(f"from_publication_date:{date_start}")
        if date_end:
            filters.append(f"to_publication_date:{date_end}")
        if filters:
            params["filter"] = ",".join(filters)
        headers = {}
        if settings.openalex_api_key is not None:
            key = settings.openalex_api_key.get_secret_value()
            if key:
                headers["Authorization"] = f"Bearer {key}"
        payload = await _request_json(
            "openalex", f"{settings.openalex_base_url.rstrip('/')}/works", params, headers=headers
        )
        try:
            metadata = _OpenAlexPage.model_validate(payload)
            if metadata.meta.page not in (None, page):
                raise ValueError("Wrong returned page")
            if metadata.meta.per_page not in (None, PAGE_SIZE):
                raise ValueError("Wrong returned page size")
            count = metadata.meta.count
            _validate_page_count(count, len(metadata.results), page)
            has_more = count > page * PAGE_SIZE
            capped = page == OPENALEX_MAX_PAGE and has_more
            warnings = []
            if capped:
                warnings.append(
                    f"OpenAlex basic paging stops at page {OPENALEX_MAX_PAGE} "
                    f"({OPENALEX_MAX_PAGE * PAGE_SIZE:,} accessible results at {PAGE_SIZE} per page). "
                    "More OpenAlex matches remain in the reported count. "
                    "Narrow the query or filters to see them."
                )
            return CatalogSearchResponse(
                count=count,
                page=page,
                next_page=page + 1 if has_more and not capped else None,
                previous_page=page - 1 if page > 1 else None,
                results=[work.result() for work in metadata.results],
                warnings=warnings,
            )
        except ValueError as exc:
            raise _invalid_metadata("openalex") from exc
