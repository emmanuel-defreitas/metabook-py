# metabook-py

Book Structure API — analyses the structural metadata of books. Given a title, ISBN, or Gutenberg ID, it locates the book via [Gutendex](https://gutendex.com), downloads and cleans the text, detects its structural schema (scripture, sectioned book, standard book, essay collection, or flat), and returns counts of chapters, paragraphs, sentences, and words per node. A `detail` parameter (`paragraph` | `sentence` | `clause` | `word`) optionally nests sentence, clause, and word nodes beneath each paragraph — still counts and positions only (a word node is just its index). You can also upload your own EPUB: `POST /api/books/upload` stores the file in Vercel Blob storage (under `books/`), walks the EPUB's package document and spine XHTML files to extract metadata and text, and returns the same structural analysis. **No book text is ever included in a response.**

Two interfaces share the same service layer:

- **REST API** (FastAPI) — `GET /api/books/structure`, `GET /api/books/structure/schemas`, `POST /api/books/upload`, `GET /health`; docs at `/api/docs`
- **MCP server** (FastMCP) — mounted at `/mcp` with tools `search_book_structure`, `upload_book_epub` (accepts the EPUB as base64 or a URL), and `list_supported_schemas`

## Quick start

```bash
make setup              # install dependencies (uv sync)
make dev                # run the API with auto-reload on :8000
make test               # run the test suite
make docker-up          # or run it via docker compose
```

EPUB uploads need a Vercel Blob read-write token in the environment (or `.env` / `.env.local`):

```bash
export BLOB_READ_WRITE_TOKEN=vercel_blob_rw_...
```

If the project is linked to Vercel, `vercel env pull` writes the token to `.env.local` (gitignored), which the app loads automatically — values there override `.env`.

CI/release pipeline scaffolded from [exegia/corpora-py](https://github.com/exegia/corpora-py): same branch model, GitHub Actions workflows, composite actions, and `make`-driven release automation.

## Branching and release

See [`.github/WORKFLOW.md`](.github/WORKFLOW.md) for the full branch model, versioning rules, and workflow reference. Quick summary:

```
<type>/<slug> --PR--> dev --(daily/manual)--> next --cut--> release/vX.Y.Z --draft PR--> main
                 (deleted on merge)         (preview)                       (deleted on release)
```

## Common commands

```bash
make help              # list all targets
make setup              # install dependencies (uv sync)
make ci                 # everything CI runs on a PR (lint + test)
make pack                # build the publishable wheel
make rulesets-diff       # rulesets GitHub currently has
make rulesets-apply      # push .github/rulesets/*.json
```

## Bootstrap

There are no `dev` / `next` branches until the first wrapup. Run the **Release** workflow manually (`Actions → Release → Run workflow`), or locally:

```bash
make bootstrap-lanes
```
