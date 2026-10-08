<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo-dark.png">
    <source media="(prefers-color-scheme: light)" srcset="assets/logo-light.png">
    <img src="assets/logo-light.png" alt="metabook-py" width="160"/>
  </picture>

  # metabook-py

  [![PR](https://img.shields.io/github/actions/workflow/status/emmanuel-defreitas/metabook-py/pr.yml?branch=dev&label=PR%20checks)](https://github.com/emmanuel-defreitas/metabook-py/actions/workflows/pr.yml)
  [![Matrix](https://img.shields.io/github/actions/workflow/status/emmanuel-defreitas/metabook-py/matrix.yml?label=matrix)](https://github.com/emmanuel-defreitas/metabook-py/actions/workflows/matrix.yml)
  [![Release](https://img.shields.io/github/v/release/emmanuel-defreitas/metabook-py?include_prereleases&label=release)](https://github.com/emmanuel-defreitas/metabook-py/releases)
  [![Python](https://img.shields.io/badge/python-3.13%2B-blue)](pyproject.toml)
  [![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

  **📚 X-ray any book's structure — chapters, paragraphs, sentences, clauses, words — as clean JSON. REST + MCP, Project Gutenberg search or your own EPUB. No book text ever leaves the API.**

  [API docs](#-rest-api) · [MCP server](#-mcp-server) · [Desktop example](#-desktop-example-gpui) · [Workflow](.github/WORKFLOW.md)

  <img src="assets/demo.gif" alt="Metabook desktop example searching Pride and Prejudice and exploring its structural schema" width="860"/>
</div>

---

## Overview

Metabook is a **Book Structure API**: give it a title, an ISBN, a Gutenberg ID, or an EPUB file, and it returns the book's *structural schema* — what kind of book it is and how it's shaped — without ever returning the text itself.

Under the hood it locates the book via [Gutendex](https://gutendex.com) (or walks your uploaded EPUB's spine), downloads and cleans the text, detects the structural schema, and counts everything at every level.

```
title / ISBN / Gutenberg ID ──► Gutendex ──► fetch + clean ──┐
                                                             ├──► detect schema ──► counts-only JSON
your .epub ──► Vercel Blob ──► package doc + spine XHTML ────┘
```

## ✨ Features

- **🔍 Fuzzy search** — find books by title or author via Gutendex, or go straight to an ISBN / Gutenberg ID
- **🧬 Schema detection** — classifies each book as `scripture`, `sectioned_book`, `standard_book`, `essay_collection`, or `flat`, with a confidence rating
- **🔢 Counts, not content** — chapters, paragraphs, sentences, clauses, and words per node; a word node is just its index. **No book text is ever included in a response**
- **📤 Bring your own EPUB** — `POST /api/books/upload` stores the file in Vercel Blob and analyses it with the same pipeline
- **🔌 Two interfaces, one service layer** — a FastAPI REST API and a FastMCP server share the same core
- **🖥️ Native desktop client** — a GPUI example app with an animated structure tree and a synced JSON code editor

### Detail levels

The `detail` parameter controls how deep the returned tree nests beneath each paragraph:

| `detail` | Nested nodes |
|----------|-----------------------------------------|
| `paragraph` | paragraphs only (counts for the rest) |
| `sentence` | + sentence nodes |
| `clause` | + clause nodes |
| `word` | + word nodes (index + position only) |

### Token counts

Pass an optional `tokenizer` query parameter naming a Hugging Face tokenizer
repository (e.g. `bert-base-uncased`) and every node in the tree carries a
token count alongside its word count — special tokens excluded, so parent
totals equal the sum of their children. The response metadata echoes the
resolved tokenizer name and its vocabulary size. The tokenizer is fetched
lazily on first use (cached on disk and in memory afterwards); an unknown
name returns `422`, a transient fetch failure on cold start returns `503`.
When the parameter is omitted, no token counts are computed and the response
is unchanged.

```bash
curl "http://127.0.0.1:8000/api/books/structure?title=Pride+and+Prejudice&tokenizer=bert-base-uncased"
```

### Text-Fabric reference identifiers

The standalone `tf_ref` package resolves and serializes durable references against any loaded
Text-Fabric corpus by reading its section hierarchy at runtime:

```python
from tf_ref import TextFabricAdapter, normalize, resolve, serialize

corpus = TextFabricAdapter(tf_app)
node = resolve("Genesis:1:1!word3", corpus)
stored = serialize(node, corpus)  # e.g. bhsa@2021/Genesis:1:1!word3
assert normalize("Genesis:1:1!word3", corpus) == stored
```

- Short grammar: `[corpus[@version]/]section[:section...][!typeN[-N]]`; canonical URNs use
  `urn:tf:corpus[@version]:section[:section...][!typeN[-N]]`.
- A heading containing `:`, `/`, `!`, `@`, whitespace, or `"` is double-quoted; embedded quotes
  are doubled, as in `"say ""hi"""`. URNs percent-encode headings.
- Positions are 1-based and ranges are inclusive. A unit belongs to the section containing its
  first text slot, including units that continue into a later section.
- Selectors on partial section paths resolve normally; serializing the resulting node emits the
  canonical innermost section because a bare node does not retain its original broader path.
- Positional identifiers are stable only within one `(corpus id, version, language)` triple.
- Current ambiguities are limited to colliding sibling headings, partial-depth selector stability,
  strict RFC treatment of URN components, missing-language fallback, and multi-section spans.

Multi-section spans are intentionally deferred. A future form can name both endpoints after `!`
without changing today's section grammar; current ranges must stay within one addressed section.

## 🚀 Quick start

```bash
make setup              # install dependencies (uv sync)
make dev                # run the API with auto-reload on :8000
make test               # run the test suite
make docker-up          # or run it via docker compose
```

Then open the interactive docs at [`http://127.0.0.1:8000/api/docs`](http://127.0.0.1:8000/api/docs), or try:

```bash
curl "http://127.0.0.1:8000/api/books/structure?title=Pride+and+Prejudice&detail=sentence"
```

### EPUB uploads

Uploads need a Vercel Blob read-write token in the environment (or `.env` / `.env.local`):

```bash
export BLOB_READ_WRITE_TOKEN=vercel_blob_rw_...
```

If the project is linked to Vercel, `vercel env pull` writes the token to `.env.local` (gitignored), which the app loads automatically — values there override `.env`.

### PostgreSQL storage

Set `DATABASE_URL` in `.env.local` (gitignored) to persist metadata and complete
structural results. The desktop app sends uploads to the API; database credentials
stay in the API configuration. Aiven connections retain `sslmode=require`:

```dotenv
DATABASE_URL="postgres://avnadmin:YOUR_PASSWORD@YOUR_HOST:22637/defaultdb?sslmode=require"
# Optional owner identifier for this single-user API instance:
DEFAULT_USER_ID="your-user-id"
```

API startup creates the additive schema in `public`: `books`, `metadata`, `author`,
`schemas`, `publisher`, and `license`, plus `metadata_authors` for ordered creators.
`books` uses quoted `userId`, `metadataId`, and `schemaId` columns; `token` is the
total token count (null when not requested), and `score` is heuristic detector
support. `userId` is nullable until an owner is configured; it does not implement
authentication. `metadata.authorId` points to the first creator, while the join
table preserves all creators. Publisher, rights/license, publication date, and
explicit `schema:numberOfPages` EPUB metadata are stored when supplied; unknown
values remain null. `metadata.LicenseId` is the license relationship.

Metadata, relationships, and the structural JSON commit in one transaction.
The database does not store extracted source prose. Uploaded EPUB files remain
in the existing private Vercel Blob store. A failed PostgreSQL write returns 503
instead of reporting a successful save. A successful response includes `record_id`;
`GET /api/books/uploads/{record_id}/structure` reopens its committed result.
`GET /health` verifies the configured database connection.

Real database tests use an isolated temporary schema and remove it afterward:

```bash
TEST_DATABASE_URL='postgres://...' uv run pytest tests/test_postgres.py -m integration
```

### Legacy uploads collection (MongoDB, optional)

Every book a user uploads or selects from search results is persisted as a
document in a MongoDB `uploads` collection: the book metadata, format
(`epub`), the Vercel Blob link, and the scan state (scanned yet, last
scanned, scope, schema, total token count). The structure tree itself is
never stored. Set `MONGODB_URI` to enable (empty = disabled, no behavior
change). `DATABASE_URL` takes precedence when both are configured. Re-selecting the same Gutenberg book updates its document instead
of duplicating it:

```bash
export MONGODB_URI=mongodb://localhost:27017
```

Browse what's stored via `GET /api/books/uploads`.

## 🌐 REST API

| Method | Endpoint | Purpose |
|--------|----------|---------|
| `GET` | `/api/books/structure` | Analyse by `title`, `isbn`, or `gutenberg_id` (+ `detail`, `tokenizer`) |
| `GET` | `/api/books/search` | Browse Gutendex metadata by `q` (title/author), `isbn`, or `gutenberg_id`; supports `page` and optional `language` |
| `GET` | `/api/books/structure/schemas` | List the supported structural schemas |
| `POST` | `/api/books/upload` | Upload an EPUB and analyse it |
| `GET` | `/api/books/uploads` | List saved books (`DATABASE_URL` or legacy `MONGODB_URI`) |
| `GET` | `/api/books/uploads/{id}/structure` | Reopen a saved PostgreSQL scan |
| `GET` | `/health` | Database readiness + cache stats |

Interactive OpenAPI docs live at `/api/docs`.

## 🤖 MCP server

A [FastMCP](https://gofastmcp.com) server is mounted at `/mcp`, so agents can use the same service layer through three tools:

| Tool | Description |
|------|-------------|
| `search_book_structure` | Analyse a book by title, ISBN, or Gutenberg ID |
| `upload_book_epub` | Analyse an EPUB passed as base64 or a URL |
| `list_supported_schemas` | Enumerate the structural schemas |

## 🖥️ Desktop example (GPUI)

[`example/`](example) is a native macOS client built with [GPUI](https://www.gpui.rs) and [gpui-component](https://github.com/longbridge/gpui-component): a sidebar workspace whose Dashboard holds fuzzy search, EPUB drag-and-drop, and a cover grid of every book the API has scanned; selecting a book gives an animated lazily-materialised structure tree beside a read-only JSON code editor (tree-sitter highlighting, folding) that scrolls to and highlights whichever node you select. Light and dark themes, spring animations, collapsible sidebar.

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/screenshot-dark.png">
    <source media="(prefers-color-scheme: light)" srcset="assets/screenshot-light.png">
    <img src="assets/screenshot-light.png" alt="Metabook desktop example — structure tree synced with the JSON schema editor" width="860"/>
  </picture>
</div>

<details>
<summary>More screenshots</summary>

| Search results | Dark mode |
|---|---|
| ![Search matches](assets/screenshot-search.png) | ![Dark mode](assets/screenshot-dark.png) |

</details>

Run it (API first, then the app):

```bash
make dev
```

```bash
cd example && cargo run
```

Or run both with one command (Ctrl-C stops both):

```bash
make serve
```

See [`example/README.md`](example/README.md) for the `Metabook.app` bundle (with app icon) and `METABOOK_API` configuration.

## 🌳 Branching and release

CI/release pipeline scaffolded from [exegia/corpora-py](https://github.com/exegia/corpora-py): same branch model, GitHub Actions workflows, composite actions, and `make`-driven release automation.

See [`.github/WORKFLOW.md`](.github/WORKFLOW.md) for the full branch model, versioning rules, and workflow reference. Quick summary:

```
<type>/<slug> --PR--> dev --(daily/manual)--> next --cut--> release/vX.Y.Z --draft PR--> main
                 (deleted on merge)         (preview)                       (deleted on release)
```

### Common commands

```bash
make help                # list all targets
make ci                  # everything CI runs on a PR (lint + test)
make pack                # build the publishable wheel
make rulesets-diff       # rulesets GitHub currently has
make rulesets-apply      # push .github/rulesets/*.json
```

### Bootstrap

There are no `dev` / `next` branches until the first wrapup. Run the **Release** workflow manually (`Actions → Release → Run workflow`), or locally:

```bash
make bootstrap-lanes
```

## 📄 License

[MIT](LICENSE) © Emmanuel De Freitas


### Explainable schema detection and cleanup

REST `GET /api/books/structure` and `POST /api/books/upload`, and MCP
`search_book_structure` and `upload_book_epub`, now include these additive
fields inside `structure`:

```json
{
  "schema": "standard_book",
  "schema_confidence": "medium",
  "schema_score": 0.5,
  "schema_detected": "standard_book",
  "schema_overridden": false,
  "schema_evidence": {
    "verse_number_lines": 0,
    "part_markers": 0,
    "chapter_word_markers": 2,
    "chapter_numeral_markers": 0,
    "caps_title_lines": 0,
    "title_byline_pairs": 0,
    "paragraph_blocks": 4
  },
  "schema_candidates": {
    "canonical_scripture": 0.0,
    "sectioned_book": 0.0,
    "standard_book": 0.5,
    "essay_or_story_collection": 0.0,
    "flat": 0.1
  }
}
```

Scores are **heuristic rule support, not calibrated probabilities**. Candidate
scores do not sum to one. Selection retains the established priority:
scripture, sectioned book, standard book, essay/story collection, then flat.
The evidence contains counts only, never matched source excerpts.

| Schema | Signals and eligibility |
| --- | --- |
| `canonical_scripture` | At least 10 line-leading chapter:verse numbers; weaker inputs score `0.03 × verse_count` |
| `sectioned_book` | At least 2 part markers and 2 explicit chapter markers; support uses their combined count |
| `standard_book` | At least 2 explicit chapter headings or standalone dotted numerals; support uses the larger count |
| `essay_or_story_collection` | At least 2 all-caps title lines excluding structural keywords; repeated title + `By …` lines strengthen support |
| `flat` | Fallback, with score 0.2 when no structural signals exist and 0.1 when weak signals exist |

For eligible marked schemas, support is `min(0.9, 0.3 + 0.1 × marker_count)`.
Chapter and title candidates also expose subthreshold support for a single
marker. Collection support is capped at 0.65 without at least two title/byline
pairs, or 0.85 with them. When multiple schema families qualify, the selected
score is capped at 0.6; part and chapter support alone describe one family.
Confidence is `high` at 0.8 or above, `medium` at 0.5 or above, otherwise `low`.
Flat fallback now reports **low**, replacing the earlier unconditional high.
These rules can still misidentify unusual formatting.

Supply `schema_override` with any schema name from `/api/books/structure/schemas`
to choose a different builder. For example, against a local server:

```bash
curl 'http://localhost:8001/api/books/structure?gutenberg_id=1342&schema_override=flat'
```

For MCP, pass the same optional argument to either analysis tool. `schema`
echoes the requested builder; `schema_detected`, confidence, score, evidence,
and candidates continue to describe automatic detection. Invalid REST values
return 422; MCP validates its enum argument (direct service calls return
`invalid_schema_override`). Python callers can use
`detect_schema(text, schema_override=SchemaType.FLAT)`.

Detection and counting share idempotent preprocessing, including EPUB analysis.
The fetcher reuses that cleanup instead of implementing a second license strip.
It recognises modern starred Gutenberg START/END markers (including wrapped
marker titles) and legacy `End of Project Gutenberg's EBook …` / Etext footers.
It removes an explicitly labelled opening contents page, and preceding short
metadata, only after finding at least two plausible entries and a body boundary.
Thus contents chapter listings no longer inflate body counts or confidence in
recognised layouts. No license boundary is guessed when markers are absent.
Unlabelled or uncertain front matter, prefaces, contents-only inputs and contents
following real prose are retained. Sparse, unusual contents layouts may remain;
HTML paragraph/heading/line-break boundaries are preserved before cleanup;
unusual markup and navigation tables may still need publisher-specific handling.

All tree indices remain **one-based ordinals within the cleaned body**, restarting
within each parent (scripture retains chapter/verse numbers). They are not byte or
character offsets into the source download. Removing a contents page changes
these body ordinals and totals; no original-source offset mapping is promised.
The existing structural-only response contract remains in place.

The in-repo `example/` client displays the complete response JSON, including
these additive fields. Before release, review adoption in
`emmanuel-defreitas/MetaBookSDK` and the MetaBook app, especially the corrected
flat confidence and changed counts. External issue creation and publication
are deferred to an authorised review/release step.

Future feedback ideas, outside this change: dialogue share per chapter with
quoted spans represented only by positions, and sentence-length distributions.

The `/health` response and OpenAPI `info.version` report the installed package
version (or `pyproject.toml` for source-only deployments), so release verification
can compare the running API with the PyPI release.
