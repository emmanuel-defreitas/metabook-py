"""Live provider metadata, separate from BookMatch and structural analysis."""

from typing import Literal

from pydantic import BaseModel, Field

CatalogSource = Literal["gutendex", "openalex"]


class CatalogResult(BaseModel):
    source: CatalogSource
    id: str
    gutenberg_id: int | None = None
    title: str
    authors: list[str] = Field(default_factory=list)
    language: str = ""
    publication_date: str | None = None
    kind: str = ""
    categories: list[str] = Field(default_factory=list)
    url: str | None = None


class CatalogSearchResponse(BaseModel):
    count: int = Field(ge=0)
    page: int = Field(ge=1)
    next_page: int | None = None
    previous_page: int | None = None
    results: list[CatalogResult] = Field(default_factory=list)
    warnings: list[str] = Field(default_factory=list)
