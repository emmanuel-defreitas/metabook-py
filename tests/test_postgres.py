"""Opt-in real PostgreSQL tests, isolated from the public library in a temporary schema."""

import os
from datetime import UTC, datetime
from uuid import uuid4

import httpx
import psycopg
import pytest
from conftest import build_epub
from psycopg import sql

from metabook_py.main import app
from metabook_py.models.book import AuthorInfo, BookInfo, UploadedBookInfo
from metabook_py.services.blob import BlobResult
from metabook_py.services.counter import build_structure_tree
from metabook_py.services.detector import detect_schema
from metabook_py.services.epub import parse_epub
from metabook_py.services.postgres import PersistenceError, PostgresUploadStore
from metabook_py.services.store import upload_doc
from metabook_py.services.tokenizers import TokenEncoder


def upload_fixture():
    parsed = parse_epub(build_epub())
    schema = detect_schema(parsed.text)
    encoder = TokenEncoder(name="test-words", vocab_size=99, encode=lambda text: len(text.split()))
    nodes, summary = build_structure_tree(parsed.text, schema, detail="sentence", encoder=encoder)
    book = UploadedBookInfo(
        title=parsed.metadata.title,
        authors=[AuthorInfo(name="Jane Austen"), AuthorInfo(name="Second Author")],
        publisher="Test Publisher",
        license="Public domain",
        date="1813",
        number_of_pages=432,
    )
    doc = upload_doc(
        book,
        BlobResult(url="https://example.test/book", pathname="books/test.epub", size=100),
        schema,
        "sentence",
        summary,
        encoder,
    )
    doc["result"] = {
        "book": book.model_dump(mode="json"),
        "blob": doc["blob"],
        "structure": {
            "schema": schema.name.value,
            "schema_confidence": schema.confidence,
            **schema.explanation(),
            "summary": summary.model_dump(),
            "nodes": [n.model_dump() for n in nodes],
        },
        "meta": {
            "uploaded_at": datetime.now(UTC).isoformat(),
            "spine_document_count": 3,
            "processing_time_ms": 1,
        },
    }
    return doc


@pytest.fixture
async def postgres_store():
    uri = os.environ.get("TEST_DATABASE_URL")
    if not uri:
        pytest.skip("Set TEST_DATABASE_URL to opt into real database tests")
    schema = f"metabook_test_{uuid4().hex}"
    async with await psycopg.AsyncConnection.connect(uri, autocommit=True) as conn:
        await conn.execute(sql.SQL("CREATE SCHEMA {}").format(sql.Identifier(schema)))
    store = PostgresUploadStore(uri, "test-user", schema=schema)
    try:
        await store.initialize()
        yield store
    finally:
        async with await psycopg.AsyncConnection.connect(uri, autocommit=True) as conn:
            await conn.execute(sql.SQL("DROP SCHEMA {} CASCADE").format(sql.Identifier(schema)))


@pytest.mark.integration
async def test_upload_relationships_result_and_reconnect(postgres_store):
    doc = upload_fixture()
    key = await postgres_store.record_upload(doc)
    # New store/connection demonstrates committed storage, not process memory.
    reopened = PostgresUploadStore(postgres_store._uri, schema=postgres_store._schema)
    result = await reopened.get_result(key)
    assert result == {**doc["result"], "record_id": key}
    assert result["structure"]["nodes"][0]["paragraphs"][0]["sentences"]
    records = await reopened.list_uploads(source="upload", limit=1)
    assert records[0]["id"] == key
    assert [a["name"] for a in records[0]["book"]["authors"]] == ["Jane Austen", "Second Author"]
    assert records[0]["book"]["publisher"] == "Test Publisher"
    assert records[0]["book"]["license"] == "Public domain"
    assert records[0]["book"]["number_of_pages"] == 432
    async with postgres_store._connection() as conn:
        cur = await conn.execute(
            'SELECT status, "userId", token, score, "metadataId", "schemaId" FROM books'
        )
        row = await cur.fetchone()
        assert row["status"] == "scanned"
        assert row["userId"] == "test-user"
        assert row["token"] == doc["scan"]["total_tokens"]
        assert row["score"] == doc["result"]["structure"]["schema_score"]
        assert row["metadataId"] and row["schemaId"]
    assert "It is a truth universally acknowledged" not in str(result)


@pytest.mark.integration
async def test_upload_transaction_rolls_back_all_relationships(postgres_store):
    doc = upload_fixture()
    doc["book"]["number_of_pages"] = -1
    with pytest.raises(PersistenceError):
        await postgres_store.record_upload(doc)
    async with postgres_store._connection() as conn:
        for table in ("books", "metadata", "schemas", "author", "publisher", "license"):
            cur = await conn.execute(
                sql.SQL("SELECT count(*) AS n FROM {}").format(sql.Identifier(table))
            )
            assert (await cur.fetchone())["n"] == 0


@pytest.mark.integration
async def test_gutenberg_rescan_preserves_identity_and_created_at(postgres_store):
    book = BookInfo(
        gutenberg_id=1342, title="Pride and Prejudice", authors=[AuthorInfo(name="Jane Austen")]
    )
    key = await postgres_store.record_gutenberg_book(book)
    pending = (await postgres_store.list_uploads())[0]
    assert pending["scan"]["scanned"] is False
    doc = upload_fixture()
    scan = {f"scan.{k}": v for k, v in doc["scan"].items()}
    scan["result"] = doc["result"]
    assert await postgres_store.record_scan(1342, scan) == key
    assert await postgres_store.record_gutenberg_book(book) == key
    assert await postgres_store.record_scan(1342, scan) == key
    records = await postgres_store.list_uploads()
    assert len(records) == 1
    assert records[0]["created_at"] == pending["created_at"]
    assert records[0]["scan"]["scanned"] is True
    assert await postgres_store.list_uploads(source="upload") == []


@pytest.mark.integration
async def test_stored_result_endpoint_roundtrip(postgres_store, monkeypatch):
    monkeypatch.setattr("metabook_py.routers.books.get_upload_store", lambda: postgres_store)
    key = await postgres_store.record_upload(upload_fixture())
    async with httpx.AsyncClient(
        transport=httpx.ASGITransport(app=app), base_url="http://test"
    ) as client:
        response = await client.get(f"/api/books/uploads/{key}/structure")
        assert response.status_code == 200
        assert response.json()["record_id"] == key
        assert response.json()["book"]["number_of_pages"] == 432
        assert (await client.get("/api/books/uploads/invalid-id/structure")).status_code == 404


async def test_database_failure_is_visible_and_safe(monkeypatch):
    class FailedStore:
        async def list_uploads(self, **kwargs):
            raise PersistenceError(
                "The database operation failed. No successful save was confirmed."
            )

    monkeypatch.setattr("metabook_py.routers.books.get_upload_store", lambda: FailedStore())
    async with httpx.AsyncClient(
        transport=httpx.ASGITransport(app=app), base_url="http://test"
    ) as client:
        response = await client.get("/api/books/uploads")
    assert response.status_code == 503
    assert response.json()["detail"]["error"] == "database_unavailable"
    assert "postgres://" not in response.text
