# metabook-py

Book Structure API — analyses the structural metadata of Project Gutenberg books. Given a title, ISBN, or Gutenberg ID, it locates the book via [Gutendex](https://gutendex.com), downloads and cleans the text, detects its structural schema (scripture, sectioned book, standard book, essay collection, or flat), and returns counts of chapters, paragraphs, sentences, and words per node. **No book text is ever included in a response.**

Two interfaces share the same service layer:

- **REST API** (FastAPI) — `GET /api/books/structure`, `GET /api/books/structure/schemas`, `GET /health`; docs at `/api/docs`
- **MCP server** (FastMCP) — mounted at `/mcp` with tools `search_book_structure` and `list_supported_schemas`

## Quick start

```bash
make setup              # install dependencies (uv sync)
make dev                # run the API with auto-reload on :8000
make test               # run the test suite
make docker-up          # or run it via docker compose
```

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
