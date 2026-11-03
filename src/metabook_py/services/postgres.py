"""Transactional PostgreSQL storage for bibliographic metadata and scan results.

The result is the public API's structural JSON, never the extracted book text.
Each operation owns its connection/transaction; failed writes roll back and are
reported to the caller. DATABASE_URL is only read by the Python API.
"""

from __future__ import annotations

import asyncio
import json
import logging
import re
from contextlib import asynccontextmanager
from datetime import UTC, datetime
from pathlib import Path
from typing import Any
from uuid import UUID, uuid4

import psycopg
from psycopg import sql
from psycopg.rows import dict_row
from psycopg.types.json import Jsonb

from metabook_py.models.book import BookInfo

logger = logging.getLogger(__name__)


class PersistenceError(Exception):
    """Safe public error: database details and credentials are never exposed."""


class PostgresUploadStore:
    enabled = True

    def __init__(self, uri: str, user_id: str | None = None, *, schema: str = "public") -> None:
        if not re.fullmatch(r"[a-z_][a-z0-9_]*", schema):
            raise ValueError("Invalid PostgreSQL schema name")
        self._uri = uri
        self._schema = schema
        self._user_id = user_id
        self._ready = False
        self._init_lock = asyncio.Lock()

    @asynccontextmanager
    async def _connection(self):
        try:
            async with await psycopg.AsyncConnection.connect(
                self._uri,
                row_factory=dict_row,
                connect_timeout=10,
                options=f"-c statement_timeout=30000 -c search_path={self._schema}",
            ) as conn:
                yield conn
        except psycopg.Error as exc:
            logger.error(
                "PostgreSQL operation failed (%s, SQLSTATE %s)", type(exc).__name__, exc.sqlstate
            )
            raise PersistenceError(
                "The database operation failed. No successful save was confirmed."
            ) from None

    async def initialize(self) -> None:
        if self._ready:
            return
        async with self._init_lock:
            if self._ready:
                return
            async with self._connection() as conn:
                # Serialize concurrent workers performing the additive bootstrap.
                await conn.execute("SELECT pg_advisory_xact_lock(742190301)")
                await conn.execute(Path(__file__).with_suffix(".sql").read_text())
            self._ready = True

    async def _named_id(self, conn, table: str, name: str | None) -> UUID | None:
        if not name:
            return None
        cur = await conn.execute(
            sql.SQL(
                "INSERT INTO {} (id, name) VALUES (%s, %s) "
                "ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name RETURNING id"
            ).format(sql.Identifier(table)),
            (uuid4(), name),
        )
        return (await cur.fetchone())["id"]

    async def _metadata(self, conn, book: dict, metadata_id: UUID) -> None:
        author_ids = []
        for author in book.get("authors", []):
            identity = json.dumps(
                [author["name"], author.get("birth_year"), author.get("death_year")]
            )
            cur = await conn.execute(
                "INSERT INTO author (id, name, birth_year, death_year, identity_key) "
                "VALUES (%s, %s, %s, %s, %s) ON CONFLICT (identity_key) "
                "DO UPDATE SET name = EXCLUDED.name RETURNING id",
                (
                    uuid4(),
                    author["name"],
                    author.get("birth_year"),
                    author.get("death_year"),
                    identity,
                ),
            )
            author_ids.append((await cur.fetchone())["id"])
        publisher_id = await self._named_id(conn, "publisher", book.get("publisher"))
        license_id = await self._named_id(conn, "license", book.get("license"))
        await conn.execute(
            'INSERT INTO metadata (id, title, "authorId", "publisherId", "LicenseId", '
            'date, "numberOfPage", language, subjects, isbn) VALUES (%s,%s,%s,%s,%s,%s,%s,%s,%s,%s) '
            'ON CONFLICT (id) DO UPDATE SET title=EXCLUDED.title, "authorId"=EXCLUDED."authorId", '
            '"publisherId"=EXCLUDED."publisherId", "LicenseId"=EXCLUDED."LicenseId", '
            'date=EXCLUDED.date, "numberOfPage"=EXCLUDED."numberOfPage", '
            "language=EXCLUDED.language, subjects=EXCLUDED.subjects, isbn=EXCLUDED.isbn",
            (
                metadata_id,
                book["title"],
                author_ids[0] if author_ids else None,
                publisher_id,
                license_id,
                book.get("date"),
                book.get("number_of_pages"),
                book.get("language", "en"),
                Jsonb(book.get("subjects", [])),
                book.get("isbn"),
            ),
        )
        await conn.execute('DELETE FROM metadata_authors WHERE "metadataId"=%s', (metadata_id,))
        for position, author_id in enumerate(author_ids):
            await conn.execute(
                'INSERT INTO metadata_authors ("metadataId", "authorId", position) VALUES (%s,%s,%s)',
                (metadata_id, author_id, position),
            )

    async def _scan(self, conn, book_id: UUID, schema_id: UUID, scan: dict, result: dict) -> None:
        now = datetime.now(UTC)
        result = {**result, "record_id": str(book_id)}
        await conn.execute(
            'INSERT INTO schemas (id, type, scope, tokenizer, scan, result, "updatedAt") '
            "VALUES (%s,%s,%s,%s,%s,%s,%s) ON CONFLICT (id) DO UPDATE SET "
            "type=EXCLUDED.type, scope=EXCLUDED.scope, tokenizer=EXCLUDED.tokenizer, "
            'scan=EXCLUDED.scan, result=EXCLUDED.result, "updatedAt"=EXCLUDED."updatedAt"',
            (
                schema_id,
                scan["schema"],
                scan["scope"],
                scan.get("tokenizer"),
                Jsonb(scan, dumps=lambda value: json.dumps(value, default=str)),
                Jsonb(result),
                now,
            ),
        )
        await conn.execute(
            "UPDATE books SET status='scanned', \"schemaId\"=%s, token=%s, score=%s, "
            '"updatedAt"=%s WHERE id=%s',
            (
                schema_id,
                scan.get("total_tokens"),
                result["structure"]["schema_score"],
                now,
                book_id,
            ),
        )

    async def record_upload(self, doc: dict[str, Any]) -> str:
        await self.initialize()
        book_id, metadata_id, schema_id = uuid4(), uuid4(), uuid4()
        async with self._connection() as conn:
            await self._metadata(conn, doc["book"], metadata_id)
            await conn.execute(
                'INSERT INTO books (id,status,"userId","metadataId",source,blob,"createdAt","updatedAt") '
                "VALUES (%s,'pending',%s,%s,'upload',%s,%s,%s)",
                (
                    book_id,
                    self._user_id,
                    metadata_id,
                    Jsonb(doc["blob"]),
                    doc["created_at"],
                    doc["updated_at"],
                ),
            )
            await self._scan(conn, book_id, schema_id, doc["scan"], doc["result"])
        return str(book_id)

    async def record_gutenberg_book(self, book: BookInfo) -> str:
        await self.initialize()
        now = datetime.now(UTC)
        async with self._connection() as conn:
            await conn.execute("SELECT pg_advisory_xact_lock(%s)", (book.gutenberg_id,))
            cur = await conn.execute(
                "SELECT id, \"metadataId\" FROM books WHERE source='gutenberg' AND gutenberg_id=%s FOR UPDATE",
                (book.gutenberg_id,),
            )
            existing = await cur.fetchone()
            book_id = existing["id"] if existing else uuid4()
            metadata_id = existing["metadataId"] if existing else uuid4()
            await self._metadata(conn, book.model_dump(), metadata_id)
            await conn.execute(
                'INSERT INTO books (id,status,"userId","metadataId",source,gutenberg_id,"createdAt","updatedAt") '
                "VALUES (%s,'pending',%s,%s,'gutenberg',%s,%s,%s) "
                'ON CONFLICT (id) DO UPDATE SET "updatedAt"=EXCLUDED."updatedAt"',
                (book_id, self._user_id, metadata_id, book.gutenberg_id, now, now),
            )
        return str(book_id)

    async def record_scan(self, gutenberg_id: int, scan_set: dict[str, Any]) -> str:
        await self.initialize()
        async with self._connection() as conn:
            cur = await conn.execute(
                "SELECT id, \"schemaId\" FROM books WHERE source='gutenberg' AND gutenberg_id=%s FOR UPDATE",
                (gutenberg_id,),
            )
            book = await cur.fetchone()
            if book is None:
                raise PersistenceError("The selected book has no database record.")
            scan = {
                k.removeprefix("scan."): v for k, v in scan_set.items() if k.startswith("scan.")
            }
            await self._scan(
                conn, book["id"], book["schemaId"] or uuid4(), scan, scan_set["result"]
            )
        return str(book["id"])

    async def list_uploads(self, *, limit: int = 100, source: str | None = None) -> list[dict]:
        await self.initialize()
        async with self._connection() as conn:
            cur = await conn.execute(
                """SELECT b.*, m.title, m.language, m.subjects, m.isbn, m.date, m."numberOfPage",
                          p.name AS publisher, l.name AS license, s.scan,
                          COALESCE((SELECT jsonb_agg(jsonb_build_object('name', a.name,
                            'birth_year', a.birth_year, 'death_year', a.death_year) ORDER BY ma.position)
                            FROM metadata_authors ma JOIN author a ON a.id=ma."authorId"
                            WHERE ma."metadataId"=m.id), '[]') AS authors
                   FROM books b JOIN metadata m ON m.id=b."metadataId"
                   LEFT JOIN publisher p ON p.id=m."publisherId"
                   LEFT JOIN license l ON l.id=m."LicenseId"
                   LEFT JOIN schemas s ON s.id=b."schemaId"
                   WHERE (%s::text IS NULL OR b.source=%s)
                   ORDER BY b."createdAt" DESC, b.id LIMIT %s""",
                (source, source, limit),
            )
            rows = await cur.fetchall()
        return [
            {
                "id": str(row["id"]),
                "source": row["source"],
                "gutenberg_id": row["gutenberg_id"],
                "format": row["format"],
                "blob": row["blob"],
                "book": {
                    k: row[k]
                    for k in (
                        "title",
                        "authors",
                        "language",
                        "subjects",
                        "isbn",
                        "publisher",
                        "license",
                        "date",
                    )
                }
                | {"number_of_pages": row["numberOfPage"]},
                "scan": row["scan"] or {"scanned": False},
                "created_at": row["createdAt"],
                "updated_at": row["updatedAt"],
            }
            for row in rows
        ]

    async def get_result(self, book_id: str) -> dict | None:
        try:
            key = UUID(book_id)
        except ValueError:
            return None
        await self.initialize()
        async with self._connection() as conn:
            cur = await conn.execute(
                'SELECT s.result FROM books b JOIN schemas s ON s.id=b."schemaId" WHERE b.id=%s',
                (key,),
            )
            row = await cur.fetchone()
        return row["result"] if row else None

    async def close(self) -> None:
        # Each operation closes its own connection, including failures.
        pass

    async def check_health(self) -> None:
        await self.initialize()
        async with self._connection() as conn:
            await conn.execute("SELECT 1")
