# Metabook PostgreSQL verification

Verified locally on 2026-10-07 against the configured Aiven `defaultdb` database.
The connection string is held only in ignored `.env.local`; no credentials are
included in this report or the application bundle.

## Observed behavior

1. Connected to PostgreSQL 18.6 and confirmed TLS through `pg_stat_ssl`.
2. Initialized the six requested tables in `public`, plus `metadata_authors` to
   preserve ordered multiple-author relationships. All requested camelCase fields
   are quoted SQL identifiers, including `metadata."LicenseId"`.
3. Built `example/Metabook.app`, launched it through Finder/LaunchServices, and
   confirmed its launcher started the Python API on `127.0.0.1:8001`.
4. Chose `/tmp/metabook-postgres-verification.epub` through the native file picker
   and pressed Analyze with sentence detail and `bert-base-uncased` token counting.
   The API returned 201 only after its PostgreSQL transaction committed.
5. Restarted the app and API. The saved book remained in Explore; clicking its
   uploaded-EPUB card loaded `/api/books/uploads/{id}/structure` with HTTP 200.
   Closing the app's sole window exited the native process and stopped its owned
   API. Opening the bundle again started a fresh window and healthy API.

## Database evidence

The clearly named verification book remains in the library for inspection:

| Field | Observed value |
| --- | --- |
| Book id | `fdd9a125-352d-4549-a053-31d216f9da9e` |
| Title | Metabook PostgreSQL verification |
| Status | scanned |
| userId | null; no default owner was configured |
| Token count | 150 |
| Detector score | 0.6; heuristic support, not a probability |
| Schema / scope | standard_book / sentence |
| Chapters | 3 |
| Creators | Metabook Verification; Second Verification Author |
| Publisher | Metabook Test Publisher |
| License | Public domain test fixture |
| Publication date | 2026-10-07 |
| Declared pages | 42 |

Direct SQL queries confirmed one book, one metadata record, one complete schema
result, two author relationships, a publisher relationship, and a license
relationship. The stored result includes the complete structural nodes, summary,
tokenizer information, and detection evidence. Extracted source prose is not
stored in PostgreSQL; the uploaded EPUB remains in the private Blob store.

Read-only inspection:

```sql
SELECT b.id, b.status, b."userId", b.token, b.score,
       m.title, m.date, m."numberOfPage", p.name AS publisher,
       l.name AS license, s.type, s.scope,
       jsonb_array_length(s.result->'structure'->'nodes') AS chapters
FROM books b
JOIN metadata m ON m.id = b."metadataId"
JOIN schemas s ON s.id = b."schemaId"
LEFT JOIN publisher p ON p.id = m."publisherId"
LEFT JOIN license l ON l.id = m."LicenseId"
WHERE b.id = 'fdd9a125-352d-4549-a053-31d216f9da9e';
```

## Automated validation

| Check | Result |
| --- | --- |
| Python suite, excluding external integration tests | 194 passed, 1 skipped, 5 deselected |
| Final store/API + live PostgreSQL tests | 16 passed |
| Native API tests | 4 passed |
| Native app build and bundle build | Passed |
| Ruff check and formatting of changed Python files | Passed |
| Mypy for persistence, API, EPUB parsing, and MCP wiring | Passed |
| Shell syntax and Git whitespace checks | Passed |
| Python wheel contains PostgreSQL DDL | Confirmed |

Both final runs include the upload-error test: database failure returns 503
and never confirms a saved id.
Real database tests create isolated temporary schemas and remove them afterward;
they do not insert fixtures into the public library. They verify full results
across new connections, ordered relationships, rejected-write rollback,
Gutenberg rescan identity, and the saved-result API.

The implementation uses Psycopg 3 asynchronous connection contexts and transaction
commit/rollback behavior documented by
[Psycopg](https://www.psycopg.org/psycopg3/docs/basic/transactions.html).

## Runtime boundaries

This is a development bundle tied to this checkout and its `.venv`. It reuses an
already healthy API and honors remote `METABOOK_API` configuration. `userId` is
nullable; setting `DEFAULT_USER_ID` identifies this API instance's owner but does
not provide user authentication. Unknown publisher, license, date, and page count
remain null rather than being inferred from chapter or word counts.

Launch from the repository root:

```bash
bash example/build-app.sh
open example/Metabook.app
```
