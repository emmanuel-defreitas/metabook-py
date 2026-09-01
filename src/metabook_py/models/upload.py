"""
Response models for the persisted "uploads" collection.

One document per book a user uploaded (POST /upload) or selected from search
results (GET /structure resolving to a single Gutenberg book). Documents carry
metadata only — the structural tree itself is never stored (it can be orders
of magnitude larger than MongoDB's 16 MB document limit); only the summary
counts and the total token count are persisted.
"""

from __future__ import annotations

from datetime import datetime

from pydantic import BaseModel, ConfigDict, Field

from metabook_py.models.book import AuthorInfo, BlobInfo
from metabook_py.models.structure import StructureSummary


class StoredBookInfo(BaseModel):
    """Book metadata as returned by Gutendex (or extracted from the EPUB)."""

    title: str
    authors: list[AuthorInfo] = Field(default_factory=list)
    language: str = "en"
    subjects: list[str] = Field(default_factory=list)
    isbn: str | None = None


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
    total_tokens: int | None = None
    tokenizer: str | None = None
    summary: StructureSummary | None = None


class UploadRecord(BaseModel):
    """A single document of the `uploads` collection, as served by the API."""

    id: str
    source: str  # "gutenberg" | "upload"
    gutenberg_id: int | None = None
    book: StoredBookInfo
    format: str = "epub"
    blob: BlobInfo | None = None
    scan: ScanInfo
    created_at: datetime
    updated_at: datetime
