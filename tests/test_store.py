"""
Tests for the MongoDB "uploads" persistence layer.

Document builders are pure and tested directly; the router wiring is tested
against a fake store (no database required); the disabled-store path (no
MONGODB_URI) is exercised implicitly by every other endpoint test.
"""

from datetime import UTC, datetime

import httpx
import pytest
import respx
from conftest import STANDARD_BOOK, build_epub

from metabook_py.core.config import settings
from metabook_py.main import app
from metabook_py.models.book import AuthorInfo, BookInfo, UploadedBookInfo
from metabook_py.models.structure import StructureSummary
from metabook_py.services.blob import BlobResult
from metabook_py.services.detector import DetectedSchema, SchemaType
from metabook_py.services.store import (
    UploadStore,
    gutenberg_book_doc,
    scan_update_doc,
    upload_doc,
)
from metabook_py.services.tokenizers import TokenEncoder

# ── Shared fixtures ────────────────────────────────────────────────────────────

BOOK = BookInfo(
    gutenberg_id=1342,
    title="Pride and Prejudice",
    authors=[AuthorInfo(name="Jane Austen", birth_year=1775, death_year=1817)],
    language="en",
    subjects=["Fiction"],
)
UPLOADED = UploadedBookInfo(
    title="Pride and Prejudice",
    authors=[AuthorInfo(name="Jane Austen")],
    language="en-GB",
    subjects=["Fiction"],
    isbn="9780141439518",
)
BLOB = BlobResult(
    url="https://example.public.blob.vercel-storage.com/books/pride-abc123.epub",
    pathname="books/pride-abc123.epub",
    size=1234,
)
SCHEMA = DetectedSchema(name=SchemaType.STANDARD_BOOK, confidence="high", markers_found=3)
SUMMARY = StructureSummary(
    total_top_level_nodes=3,
    total_paragraphs=6,
    total_sentences=12,
    total_words=200,
    total_tokens=96,
    avg_paragraphs_per_chapter=2.0,
    avg_sentences_per_paragraph=2.0,
    avg_words_per_sentence=16.0,
)
ENCODER = TokenEncoder(name="stub-tokenizer", vocab_size=5, encode=lambda text: len(text.split()))


class FakeStore:
    """In-memory UploadStore double recording every call."""

    def __init__(self) -> None:
        self.books: list[BookInfo] = []
        self.scans: list[tuple[int, dict]] = []
        self.uploads: list[dict] = []
        self.docs: list[dict] = []

    async def record_gutenberg_book(self, book: BookInfo) -> None:
        self.books.append(book)

    async def record_scan(self, gutenberg_id: int, scan_set: dict) -> None:
        self.scans.append((gutenberg_id, scan_set))

    async def record_upload(self, doc: dict) -> str | None:
        self.uploads.append(doc)
        return "fake-object-id"

    async def list_uploads(self, *, limit: int = 100, source: str | None = None) -> list[dict]:
        return self.docs[:limit]


@pytest.fixture
def client() -> httpx.AsyncClient:
    return httpx.AsyncClient(transport=httpx.ASGITransport(app=app), base_url="http://testserver")


@pytest.fixture
def blob_token(monkeypatch):
    monkeypatch.setattr(settings, "blob_read_write_token", "vercel_blob_rw_test_token")


@pytest.fixture
def fake_store(monkeypatch) -> FakeStore:
    store = FakeStore()
    monkeypatch.setattr("metabook_py.routers.books.get_upload_store", lambda: store)
    return store


@pytest.fixture
def stub_resolver(monkeypatch):
    """Resolve any tokenizer name to a whitespace-counting stub encoder."""
    from metabook_py.services.tokenizers import get_encoder

    def fake_get_encoder(name: str):
        return ENCODER

    monkeypatch.setattr("metabook_py.routers.books.get_encoder", fake_get_encoder)
    return get_encoder


# ── Document builders ──────────────────────────────────────────────────────────


def test_gutenberg_book_doc_creates_unscanned_record():
    spec = gutenberg_book_doc(BOOK)

    assert spec["filter"] == {"source": "gutenberg", "gutenberg_id": 1342}
    assert spec["upsert"] is True

    insert = spec["update"]["$setOnInsert"]
    assert insert["scan"] == {"scanned": False}
    assert insert["blob"] is None
    assert isinstance(insert["created_at"], datetime)

    set_ = spec["update"]["$set"]
    assert set_["source"] == "gutenberg"
    assert set_["format"] == "epub"
    assert set_["book"]["title"] == "Pride and Prejudice"
    assert set_["book"]["authors"][0]["name"] == "Jane Austen"
    assert isinstance(set_["updated_at"], datetime)


def test_scan_update_doc_carries_scope_schema_and_tokens():
    scan = scan_update_doc(SCHEMA, "sentence", SUMMARY, ENCODER)

    assert scan["scan.scanned"] is True
    assert isinstance(scan["scan.last_scanned_at"], datetime)
    assert scan["scan.scope"] == "sentence"
    assert scan["scan.schema"] == "standard_book"
    assert scan["scan.schema_confidence"] == "high"
    assert scan["scan.total_tokens"] == 96  # from SUMMARY below
    assert scan["scan.tokenizer"] == "stub-tokenizer"
    assert scan["scan.summary"]["total_words"] == 200


def test_scan_update_doc_without_tokenizer_omits_token_scheme():
    summary = StructureSummary(
        total_top_level_nodes=1,
        total_paragraphs=1,
        total_sentences=1,
        total_words=10,
        avg_paragraphs_per_chapter=1.0,
        avg_sentences_per_paragraph=1.0,
        avg_words_per_sentence=10.0,
    )
    scan = scan_update_doc(SCHEMA, "paragraph", summary, None)

    assert scan["scan.total_tokens"] is None
    assert scan["scan.tokenizer"] is None


def test_upload_doc_is_complete_and_scanned():
    doc = upload_doc(UPLOADED, BLOB, SCHEMA, "word", SUMMARY, ENCODER)

    assert doc["source"] == "upload"
    assert doc["gutenberg_id"] is None
    assert doc["format"] == "epub"
    assert doc["blob"]["url"] == BLOB.url
    assert doc["blob"]["size_bytes"] == 1234
    assert doc["book"]["isbn"] == "9780141439518"

    scan = doc["scan"]
    assert scan["scanned"] is True
    assert isinstance(scan["last_scanned_at"], datetime)
    assert scan["scope"] == "word"
    assert scan["schema"] == "standard_book"
    assert scan["total_tokens"] == 96
    assert scan["tokenizer"] == "stub-tokenizer"
    assert scan["summary"]["total_paragraphs"] == 6

    assert isinstance(doc["created_at"], datetime)
    assert isinstance(doc["updated_at"], datetime)


# ── Disabled store (no MONGODB_URI) ────────────────────────────────────────────


async def test_disabled_store_is_silent_noop():
    store = UploadStore("", "metabook")

    assert store.enabled is False
    await store.record_gutenberg_book(BOOK)  # must not raise
    await store.record_scan(1342, scan_update_doc(SCHEMA, "word", SUMMARY, None))
    assert (
        await store.record_upload(upload_doc(UPLOADED, BLOB, SCHEMA, "word", SUMMARY, None)) is None
    )
    assert await store.list_uploads() == []


# ── Router wiring ──────────────────────────────────────────────────────────────

BLOB_URL_PATTERN = f"{settings.blob_api_url}/books/"
GUTENDEX_URL = f"{settings.gutendex_base_url}/books/"


def _mock_blob_ok() -> respx.Route:
    return respx.put(url__startswith=BLOB_URL_PATTERN).mock(
        return_value=httpx.Response(
            200,
            json={
                "url": "https://example.public.blob.vercel-storage.com/books/pride-xyz.epub",
                "pathname": "books/pride-xyz.epub",
                "contentType": "application/epub+zip",
            },
        )
    )


def _mock_gutendex_single(gutenberg_id: int) -> respx.Route:
    return respx.get(url__startswith=GUTENDEX_URL).mock(
        return_value=httpx.Response(
            200,
            json={
                "results": [
                    {
                        "id": gutenberg_id,
                        "title": "Pride and Prejudice",
                        "authors": [{"name": "Jane Austen", "birth_year": 1775}],
                        "languages": ["en"],
                        "subjects": ["Fiction"],
                        "formats": {
                            "text/plain; charset=utf-8": f"https://www.gutenberg.org/cache/epub/{gutenberg_id}/pg{gutenberg_id}.txt"
                        },
                    }
                ]
            },
        )
    )


def _mock_gutenberg_text(gutenberg_id: int) -> respx.Route:
    return respx.get(
        url=f"https://www.gutenberg.org/cache/epub/{gutenberg_id}/pg{gutenberg_id}.txt"
    ).mock(return_value=httpx.Response(200, text=STANDARD_BOOK))


@respx.mock
async def test_upload_endpoint_records_upload_document(
    client, blob_token, fake_store, stub_resolver
):
    _mock_blob_ok()
    epub = build_epub()

    resp = await client.post(
        "/api/books/upload?tokenizer=stub",
        files={"file": ("pride.epub", epub, "application/epub+zip")},
    )

    assert resp.status_code == 201
    assert len(fake_store.uploads) == 1

    doc = fake_store.uploads[0]
    assert doc["source"] == "upload"
    assert doc["format"] == "epub"
    assert doc["blob"]["pathname"].startswith("books/")
    assert doc["scan"]["scanned"] is True
    assert doc["scan"]["scope"] == "paragraph"
    assert doc["scan"]["schema"] == "standard_book"
    assert doc["scan"]["tokenizer"] == "stub-tokenizer"
    assert doc["scan"]["total_tokens"] > 0


@respx.mock
async def test_structure_endpoint_records_book_then_scan(client, fake_store):
    _mock_gutendex_single(999001)
    _mock_gutenberg_text(999001)

    resp = await client.get("/api/books/structure", params={"title": "Pride and Prejudice"})

    assert resp.status_code == 200
    assert [b.gutenberg_id for b in fake_store.books] == [999001]
    assert len(fake_store.scans) == 1

    gutenberg_id, scan = fake_store.scans[0]
    assert gutenberg_id == 999001
    assert scan["scan.scanned"] is True
    assert scan["scan.scope"] == "paragraph"
    assert scan["scan.schema"] == "standard_book"
    assert scan["scan.total_tokens"] is None  # no tokenizer requested
    assert scan["scan.summary"]["total_words"] > 0


@respx.mock
async def test_text_fetch_failure_leaves_unscanned_record(client, fake_store):
    _mock_gutendex_single(999002)
    respx.get(url__startswith="https://www.gutenberg.org/").mock(return_value=httpx.Response(500))

    resp = await client.get("/api/books/structure", params={"gutenberg_id": 999002})

    assert resp.status_code == 422  # text_unavailable
    assert [b.gutenberg_id for b in fake_store.books] == [999002]  # but still recorded
    assert fake_store.scans == []  # and never marked scanned


async def test_uploads_list_endpoint_serves_stored_docs(client, fake_store):
    fake_store.docs = [
        {
            "id": "664f1a2b3c4d5e6f70819293",
            "source": "upload",
            "gutenberg_id": None,
            "book": {"title": "Pride and Prejudice", "authors": [], "language": "en"},
            "format": "epub",
            "blob": {
                "url": "https://example.public.blob.vercel-storage.com/books/pride-xyz.epub",
                "pathname": "books/pride-xyz.epub",
                "size_bytes": 1234,
            },
            "scan": {"scanned": False},
            "created_at": datetime.now(UTC),
            "updated_at": datetime.now(UTC),
        }
    ]

    resp = await client.get("/api/books/uploads")

    assert resp.status_code == 200
    records = resp.json()
    assert len(records) == 1
    assert records[0]["id"] == "664f1a2b3c4d5e6f70819293"
    assert records[0]["source"] == "upload"
    assert records[0]["format"] == "epub"
    assert records[0]["scan"]["scanned"] is False
    assert records[0]["blob"]["size_bytes"] == 1234


async def test_uploads_list_rejects_unknown_source(client):
    resp = await client.get("/api/books/uploads", params={"source": "nonsense"})
    assert resp.status_code == 422
