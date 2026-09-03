"""
MongoDB persistence for the "uploads" collection.

One document is created each time a user uploads an EPUB or selects a book
from the search results:

    {
      "_id":        ObjectId,
      "source":     "gutenberg" | "upload",
      "gutenberg_id": 1342,                # null for file uploads
      "book":       { title, authors, language, subjects, isbn },
      "format":     "epub",
      "blob":       { url, pathname, size_bytes } | null,
      "scan": {
        "scanned":          bool,          # false until a structure scan ran
        "last_scanned_at":  date | null,
        "scope":            "paragraph" | "sentence" | "clause" | "word" | null,
        "schema":           "standard_book" | … | null,
        "schema_confidence":"high" | … | null,
        "total_tokens":     int | null,    # null unless a tokenizer was used
        "tokenizer":        str | null,    # scheme that produced total_tokens
        "summary":          StructureSummary | null,
      },
      "created_at": date,
      "updated_at": date,
    }

Design notes
------------
- Metadata and scan state are always read together (library views), so they
  live in one embedded document — no lookups.
- The full structure tree is NEVER stored: responses can reach hundreds of
  MB, far beyond MongoDB's 16 MB document limit. Only summary counts are.
- Gutenberg selections upsert on a unique partial index over `gutenberg_id`
  (re-selecting a book refreshes its scan instead of duplicating it); every
  file upload inserts a new document (each is a distinct blob).
- Persistence is opt-in (MONGODB_URI) and best-effort: a store failure is
  logged and never breaks an analysis request.
"""

from __future__ import annotations

import logging
from datetime import UTC, datetime
from typing import Any

from pymongo import AsyncMongoClient, IndexModel
from pymongo.asynchronous.collection import AsyncCollection
from pymongo.errors import CollectionInvalid, PyMongoError

from metabook_py.core.config import settings
from metabook_py.models.book import BlobInfo, BookInfo, UploadedBookInfo
from metabook_py.models.structure import StructureSummary
from metabook_py.services.blob import BlobResult
from metabook_py.services.detector import DetectedSchema
from metabook_py.services.tokenizers import TokenEncoder

logger = logging.getLogger(__name__)

UPLOADS_COLLECTION = "uploads"

# ── Schema validation ──────────────────────────────────────────────────────────
#
# Strict $jsonSchema so malformed documents are rejected at the database level.
# Nullable fields use ["<type>", "null"] unions; enums include null the same way.

UPLOADS_VALIDATOR: dict[str, Any] = {
    "$jsonSchema": {
        "bsonType": "object",
        "required": ["source", "book", "format", "scan", "created_at", "updated_at"],
        "properties": {
            "source": {"enum": ["gutenberg", "upload"]},
            "gutenberg_id": {"bsonType": ["int", "null"]},
            "book": {
                "bsonType": "object",
                "required": ["title"],
                "properties": {
                    "title": {"bsonType": "string"},
                    "authors": {
                        "bsonType": "array",
                        "items": {
                            "bsonType": "object",
                            "required": ["name"],
                            "properties": {
                                "name": {"bsonType": "string"},
                                "birth_year": {"bsonType": ["int", "null"]},
                                "death_year": {"bsonType": ["int", "null"]},
                            },
                        },
                    },
                    "language": {"bsonType": "string"},
                    "subjects": {"bsonType": "array", "items": {"bsonType": "string"}},
                    "isbn": {"bsonType": ["string", "null"]},
                },
            },
            "format": {"enum": ["epub"]},
            "blob": {
                "bsonType": ["object", "null"],
                "required": ["url", "pathname", "size_bytes"],
                "properties": {
                    "url": {"bsonType": "string"},
                    "pathname": {"bsonType": "string"},
                    "size_bytes": {"bsonType": "int"},
                },
            },
            "scan": {
                "bsonType": "object",
                "required": ["scanned"],
                "properties": {
                    "scanned": {"bsonType": "bool"},
                    "last_scanned_at": {"bsonType": ["date", "null"]},
                    "scope": {"enum": ["paragraph", "sentence", "clause", "word", None]},
                    "schema": {"bsonType": ["string", "null"]},
                    "schema_confidence": {"bsonType": ["string", "null"]},
                    "total_tokens": {"bsonType": ["int", "null"]},
                    "tokenizer": {"bsonType": ["string", "null"]},
                    "summary": {"bsonType": ["object", "null"]},
                },
            },
            "created_at": {"bsonType": "date"},
            "updated_at": {"bsonType": "date"},
        },
    }
}

# Unique only over Gutenberg-sourced docs: file uploads have no gutenberg_id
# (multiple uploads of the same book must all be kept), while re-selecting a
# Gutenberg book upserts into its existing document.
_GUTENBERG_ID_INDEX = IndexModel(
    {"gutenberg_id": 1},
    name="gutenberg_id_unique",
    unique=True,
    partialFilterExpression={"source": "gutenberg"},
)
_CREATED_AT_INDEX = IndexModel({"created_at": -1}, name="created_at_desc")


# ── Document builders (pure — unit-tested without a database) ──────────────────


def gutenberg_book_doc(book: BookInfo) -> dict[str, Any]:
    """Upsert filter + update for a book selected from search results.

    Created unscanned at selection time so a failed text fetch still leaves
    `scan.scanned: false` behind; a later successful scan updates the block.
    """
    now = datetime.now(UTC)
    return {
        "filter": {"source": "gutenberg", "gutenberg_id": book.gutenberg_id},
        "update": {
            "$set": {
                "source": "gutenberg",
                "gutenberg_id": book.gutenberg_id,
                "book": book.model_dump(),
                "format": "epub",
                "updated_at": now,
            },
            "$setOnInsert": {
                "blob": None,
                "scan": {"scanned": False},
                "created_at": now,
            },
        },
        "upsert": True,
    }


def scan_update_doc(
    schema: DetectedSchema,
    scope: str,
    summary: StructureSummary,
    encoder: TokenEncoder | None,
) -> dict[str, Any]:
    """`$set` payload marking a document as scanned, with its scan results."""
    return {
        "scan.scanned": True,
        "scan.last_scanned_at": datetime.now(UTC),
        "scan.scope": scope,
        "scan.schema": schema.name.value,
        "scan.schema_confidence": schema.confidence,
        "scan.total_tokens": summary.total_tokens,
        "scan.tokenizer": encoder.name if encoder else None,
        "scan.summary": summary.model_dump(),
        "updated_at": datetime.now(UTC),
    }


def upload_doc(
    book: UploadedBookInfo,
    blob: BlobResult,
    schema: DetectedSchema,
    scope: str,
    summary: StructureSummary,
    encoder: TokenEncoder | None,
) -> dict[str, Any]:
    """A complete document for a freshly uploaded + analysed EPUB."""
    now = datetime.now(UTC)
    return {
        "source": "upload",
        "gutenberg_id": None,
        "book": book.model_dump(),
        "format": "epub",
        "blob": BlobInfo(url=blob.url, pathname=blob.pathname, size_bytes=blob.size).model_dump(),
        "scan": {
            "scanned": True,
            "last_scanned_at": now,
            "scope": scope,
            "schema": schema.name.value,
            "schema_confidence": schema.confidence,
            "total_tokens": summary.total_tokens,
            "tokenizer": encoder.name if encoder else None,
            "summary": summary.model_dump(),
        },
        "created_at": now,
        "updated_at": now,
    }


# ── Store ──────────────────────────────────────────────────────────────────────


def _public_view(doc: dict[str, Any]) -> dict[str, Any]:
    """Shape a raw Mongo document for the API (ObjectId → str, drop _id)."""
    view = dict(doc)
    view["id"] = str(view.pop("_id"))
    return view


class UploadStore:
    """Best-effort async persistence for the `uploads` collection.

    Every public method is a no-op when the store is disabled (no
    MONGODB_URI) and swallows PyMongo errors after logging them — analyses
    must never fail because the database is down.
    """

    def __init__(self, uri: str, db_name: str) -> None:
        self._enabled = bool(uri)
        if self._enabled:
            self._client: AsyncMongoClient | None = AsyncMongoClient(uri)
            self._collection: AsyncCollection = self._client[db_name][UPLOADS_COLLECTION]
            self._ensured = False
        else:
            self._client = None
            self._collection = None  # type: ignore[assignment]
            self._ensured = True

    @property
    def enabled(self) -> bool:
        return self._enabled

    async def _ensure_indexes(self) -> AsyncCollection:
        """Create the collection with its validator and indexes (idempotent)."""
        assert self._collection is not None
        if not self._ensured:
            db = self._collection.database
            try:
                await db.create_collection(
                    UPLOADS_COLLECTION, validator=UPLOADS_VALIDATOR, validationLevel="strict"
                )
            except CollectionInvalid:
                # Exists already (possibly from an older deploy) — re-apply.
                await db.command(
                    "collMod",
                    UPLOADS_COLLECTION,
                    validator=UPLOADS_VALIDATOR,
                    validationLevel="strict",
                )
            await self._collection.create_indexes([_GUTENBERG_ID_INDEX, _CREATED_AT_INDEX])
            self._ensured = True
        return self._collection

    async def record_gutenberg_book(self, book: BookInfo) -> None:
        """Create/refresh the document for a book selected from search results."""
        if not self._enabled:
            return
        try:
            col = await self._ensure_indexes()
            spec = gutenberg_book_doc(book)
            await col.update_one(spec["filter"], spec["update"], upsert=spec["upsert"])
        except PyMongoError as exc:
            logger.warning("uploads: couldn't record gutenberg book %s: %s", book.gutenberg_id, exc)

    async def record_scan(self, gutenberg_id: int, scan_set: dict[str, Any]) -> None:
        """Attach scan results to an existing Gutenberg document."""
        if not self._enabled:
            return
        try:
            col = await self._ensure_indexes()
            await col.update_one(
                {"source": "gutenberg", "gutenberg_id": gutenberg_id}, {"$set": scan_set}
            )
        except PyMongoError as exc:
            logger.warning("uploads: couldn't record scan for book %s: %s", gutenberg_id, exc)

    async def record_upload(self, doc: dict[str, Any]) -> str | None:
        """Insert a new document for an uploaded + analysed EPUB; return its id."""
        if not self._enabled:
            return None
        try:
            col = await self._ensure_indexes()
            result = await col.insert_one(doc)
            return str(result.inserted_id)
        except PyMongoError as exc:
            logger.warning("uploads: couldn't record upload: %s", exc)
            return None

    async def list_uploads(
        self, *, limit: int = 100, source: str | None = None
    ) -> list[dict[str, Any]]:
        """Stored documents, newest first (metadata only, no tree data)."""
        if not self._enabled:
            return []
        try:
            col = await self._ensure_indexes()
            query = {"source": source} if source else {}
            cursor = col.find(query).sort("created_at", -1).limit(limit)
            return [_public_view(doc) async for doc in cursor]
        except PyMongoError as exc:
            logger.warning("uploads: couldn't list uploads: %s", exc)
            return []

    async def close(self) -> None:
        if self._client is not None:
            await self._client.close()


# ── Module-level singleton ─────────────────────────────────────────────────────

_store: UploadStore | None = None


def get_upload_store() -> UploadStore:
    """Lazily build the process-wide store from current settings."""
    global _store
    if _store is None:
        _store = UploadStore(settings.mongodb_uri, settings.mongodb_db)
    return _store


def reset_upload_store() -> None:
    """Drop the singleton (tests reconfigure settings between cases)."""
    global _store
    _store = None
