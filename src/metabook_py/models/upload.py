"""
Response models for the persisted "uploads" collection.

Library metadata is stored separately from complete parsed results. MongoDB
GridFS holds the full tree while this model serves compact library cards.
"""

from __future__ import annotations

from datetime import datetime

from pydantic import BaseModel, ConfigDict, Field

from metabook_py.models.book import AuthorInfo, BlobInfo, OpenLibraryMetadata
from metabook_py.models.structure import StructureSummary


class StoredBookInfo(BaseModel):
    """Book metadata as returned by Gutendex (or extracted from the EPUB)."""

    open_library: OpenLibraryMetadata | None = None
    title: str
    authors: list[AuthorInfo] = Field(default_factory=list)
    language: str = "en"
    subjects: list[str] = Field(default_factory=list)
    isbn: str | None = None
    publisher: str | None = None
    license: str | None = None
    date: str | None = None
    number_of_pages: int | None = None


class ScanInfo(BaseModel):
    """Schema-scan state for a stored book."""

    model_config = ConfigDict(populate_by_name=True)

    scanned: bool = False
    last_scanned_at: datetime | None = None
    scope: str | None = None
    # `schema` is reserved by pydantic's BaseModel, so the field is schema_type
    # with a serialization alias — the wire/storage name stays "schema".
    schema_type: str | None = Field(None, alias="schema")
    schema_confidence: str | None = None
    schema_score: float | None = None
    total_tokens: int | None = None
    tokenizer: str | None = None
    summary: StructureSummary | None = None


class UploadRecord(BaseModel):
    """A single document of the `uploads` collection, as served by the API."""

    id: str
    kind: str = "book"
    source: str  # "gutenberg" | "upload"
    gutenberg_id: int | None = None
    book: StoredBookInfo
    format: str = "epub"
    blob: BlobInfo | None = None
    scan: ScanInfo
    created_at: datetime
    updated_at: datetime
