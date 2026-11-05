"""Owner-scoped Mongo library with complete results stored in GridFS."""

import json
from datetime import UTC, datetime
from uuid import uuid4

from gridfs import AsyncGridFSBucket
from gridfs.errors import NoFile
from pymongo import AsyncMongoClient
from pymongo.errors import PyMongoError

from metabook_py.core.identity import browser_user_id
from metabook_py.services.postgres import PersistenceError


class UserLibraryStore:
    def __init__(self, uri: str, db_name: str):
        self.enabled = bool(uri)
        self.client = AsyncMongoClient(uri, serverSelectionTimeoutMS=5000)
        self.db = self.client[db_name]
        self.collection = self.db.user_library
        self.bucket = AsyncGridFSBucket(self.db, bucket_name="parsed_results")
        self._ready = False

    async def initialize(self):
        if self._ready:
            return
        try:
            await self.collection.create_index([("user_id", 1), ("created_at", -1)])
            await self.collection.create_index(
                [("user_id", 1), ("gutenberg_id", 1)],
                unique=True,
                partialFilterExpression={"source": "gutenberg"},
            )
            self._ready = True
        except PyMongoError as exc:
            raise PersistenceError("Could not initialize the saved library") from exc

    async def check_health(self):
        try:
            await self.db.command("ping")
        except PyMongoError as exc:
            raise PersistenceError("Saved library is unavailable") from exc

    async def close(self):
        await self.client.close()

    async def record_gutenberg_book(self, book):
        # Save only complete analyses; an unsuccessful scan is not a saved book.
        return None

    async def record_scan(self, gutenberg_id, scan_set):
        result = scan_set.get("result")
        if not result:
            return None
        now = datetime.now(UTC)
        doc = {
            "source": "gutenberg",
            "gutenberg_id": gutenberg_id,
            "book": result["book"],
            "format": "epub",
            "blob": None,
            "scan": {
                k.removeprefix("scan."): v for k, v in scan_set.items() if k.startswith("scan.")
            },
            "created_at": now,
            "updated_at": now,
            "result": result,
        }
        return await self._save(doc, gutenberg_id)

    async def record_upload(self, doc):
        return await self._save(doc)

    async def record_document(self, result):
        data = result.model_dump(mode="json")
        info = data["document"]
        now = datetime.now(UTC)
        doc = {
            "source": "document",
            "gutenberg_id": None,
            "kind": "document",
            "book": {
                "title": info.get("filename") or info.get("source_url") or "Untitled document",
                "authors": [],
                "subjects": [],
            },
            "format": info["format"],
            "blob": None,
            "scan": {"scanned": True, "last_scanned_at": now, "schema": "document"},
            "created_at": now,
            "updated_at": now,
            "result": data,
        }
        return await self._save(doc)

    async def _save(self, doc, gutenberg_id=None):
        owner = browser_user_id.get()
        if not owner:
            return None
        await self.initialize()
        file_id = None
        try:
            query = {"user_id": owner, "source": "gutenberg", "gutenberg_id": gutenberg_id}
            previous = await self.collection.find_one(query) if gutenberg_id is not None else None
            record_id = previous["_id"] if previous else str(uuid4())
            result = {**doc["result"], "record_id": record_id}
            file_id = await self.bucket.upload_from_stream(
                f"{record_id}.json",
                json.dumps(result, ensure_ascii=False).encode(),
                metadata={
                    "user_id": owner,
                    "record_id": record_id,
                    "contentType": "application/json",
                },
            )
            record = {k: v for k, v in doc.items() if k != "result"}
            record.update(_id=record_id, user_id=owner, result_id=file_id)
            if previous:
                record["created_at"] = previous["created_at"]
                await self.collection.replace_one({"_id": record_id, "user_id": owner}, record)
            else:
                await self.collection.insert_one(record)
        except PyMongoError as exc:
            if file_id is not None:
                await self.bucket.delete(file_id)
            raise PersistenceError("Could not save the parsed result") from exc
        # Retain earlier GridFS versions: concurrent readers may still use them.
        return record_id

    async def list_uploads(self, *, limit=100, source=None):
        owner = browser_user_id.get()
        if not owner:
            return []
        query = {"user_id": owner}
        if source:
            query["source"] = source
        try:
            cursor = (
                self.collection.find(query, {"user_id": 0, "result_id": 0})
                .sort("created_at", -1)
                .limit(limit)
            )
            return [
                {**{k: v for k, v in doc.items() if k != "_id"}, "id": doc["_id"]}
                async for doc in cursor
            ]
        except PyMongoError as exc:
            raise PersistenceError("Could not load the saved library") from exc

    async def get_result(self, book_id):
        owner = browser_user_id.get()
        if not owner:
            return None
        try:
            doc = await self.collection.find_one({"_id": book_id, "user_id": owner})
            if not doc:
                return None
            stream = await self.bucket.open_download_stream(doc["result_id"])
            return json.loads(await stream.read())
        except (PyMongoError, NoFile, ValueError) as exc:
            raise PersistenceError("Could not load the saved result") from exc
