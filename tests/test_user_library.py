from unittest.mock import AsyncMock, MagicMock
from uuid import uuid4

import httpx
import pytest
from pymongo.errors import DuplicateKeyError

from metabook_py.core.identity import browser_user_id
from metabook_py.main import app
from metabook_py.services.user_library import UserLibraryStore


@pytest.fixture
def store():
    instance = UserLibraryStore.__new__(UserLibraryStore)
    instance.enabled = True
    instance._ready = True
    instance.collection = MagicMock()
    instance.collection.find_one = AsyncMock(return_value=None)
    instance.collection.insert_one = AsyncMock()
    instance.collection.replace_one = AsyncMock()
    instance.bucket = MagicMock()
    instance.bucket.upload_from_stream = AsyncMock(return_value="file-id")
    instance.bucket.open_download_stream = AsyncMock()
    instance.bucket.delete = AsyncMock()
    return instance


async def test_anonymous_cannot_read_or_write(store):
    assert await store.list_uploads() == []
    assert await store.get_result("other") is None
    assert await store.record_upload({}) is None
    store.collection.find_one.assert_not_called()


async def test_save_complete_result_and_metadata(store):
    import json

    owner = str(uuid4())
    token = browser_user_id.set(owner)
    try:
        result = {
            "book": {"title": "Test", "authors": [{"name": "Author"}], "subjects": ["tag"]},
            "structure": {"schema": "standard_book", "nodes": [{"paragraphs": [1, 2]}]},
        }
        key = await store.record_upload(
            {"result": result, "book": result["book"], "scan": {"schema": "standard_book"}}
        )
        saved = store.collection.insert_one.call_args.args[0]
        assert saved["user_id"] == owner
        assert saved["book"]["subjects"] == ["tag"]
        assert saved["result_id"] == "file-id"
        payload = json.loads(store.bucket.upload_from_stream.call_args.args[1])
        assert payload == {**result, "record_id": key}
    finally:
        browser_user_id.reset(token)


async def test_first_save_race_updates_the_winning_record(store):
    """Two tabs saving the same new book: the loser updates the winner's record."""
    from datetime import UTC, datetime

    winner = {"_id": "winner", "created_at": datetime.now(UTC)}
    store.collection.find_one.side_effect = [None, winner]
    store.collection.insert_one.side_effect = DuplicateKeyError("dup")
    store.bucket.upload_from_stream.side_effect = ["lost-file", "kept-file"]
    token = browser_user_id.set(str(uuid4()))
    try:
        result = {"book": {"title": "Test"}, "structure": {"nodes": []}}
        key = await store._save({"result": result, "book": result["book"]}, gutenberg_id=84)
        assert key == "winner"
        store.bucket.delete.assert_awaited_once_with("lost-file")
        saved = store.collection.replace_one.call_args.args[1]
        assert saved["result_id"] == "kept-file"
        assert saved["created_at"] == winner["created_at"]
    finally:
        browser_user_id.reset(token)


async def test_cross_user_result_never_reads_gridfs(store):
    token = browser_user_id.set(str(uuid4()))
    try:
        assert await store.get_result("someone-elses-record") is None
        store.collection.find_one.assert_awaited_once_with(
            {"_id": "someone-elses-record", "user_id": browser_user_id.get()}
        )
        store.bucket.open_download_stream.assert_not_called()
    finally:
        browser_user_id.reset(token)


async def test_owned_result_reopens(store):
    token = browser_user_id.set(str(uuid4()))
    store.collection.find_one.return_value = {"result_id": "file-id"}
    stream = AsyncMock()
    stream.read.return_value = b'{"record_id":"saved", "structure":{"nodes":[1]}}'
    store.bucket.open_download_stream.return_value = stream
    try:
        assert (await store.get_result("saved"))["structure"]["nodes"] == [1]
    finally:
        browser_user_id.reset(token)


async def test_invalid_uuid_rejected_and_context_reset():
    async with httpx.AsyncClient(
        transport=httpx.ASGITransport(app=app), base_url="http://test"
    ) as client:
        response = await client.get("/api/books/uploads", headers={"X-Metabook-User-ID": "invalid"})
        assert response.status_code == 400
    assert browser_user_id.get() is None


async def test_uuid_does_not_fall_back_to_shared_postgres():
    async with httpx.AsyncClient(
        transport=httpx.ASGITransport(app=app), base_url="http://test"
    ) as client:
        response = await client.get(
            "/api/books/uploads", headers={"X-Metabook-User-ID": str(uuid4())}
        )
        assert response.status_code == 503
        assert response.headers["cache-control"] == "private, no-store"
