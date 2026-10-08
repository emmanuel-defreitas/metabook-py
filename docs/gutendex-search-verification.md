# Gutendex search verification

Verified locally on 2026-10-07 with the native Metabook app and its PostgreSQL API.

## Behavior

- `/api/books/search` returns metadata pages with total count, page number, and
  optional next/previous page numbers. Titles and author names use Gutendex's
  `search` parameter; Gutenberg IDs use `/books/<id>/`.
- The desktop search shows results before analysis, including single matches.
  Previous/Next retain the submitted query. Selecting a result runs the existing
  structural analysis and persistence flow.
- Browsing does not fetch book text, tokenize, or write database records. All
  languages are included by default; REST callers can supply `language`.
- ISBN remains a best-effort keyword search because Gutendex has no ISBN index.
  Empty results remain an ordinary search result. Upstream failures return
  502/504; books without text/HTML return an actionable analysis error.

## Evidence

- Native author search for `Austen` returned 49 real results, with Next available.
- Page two returned 17 remaining results from the running API after retry.
- Native ID search for `1342` returned Pride and Prejudice. Selecting Schema
  opened its metadata and structural result: standard_book, 61 chapters,
  2,310 paragraphs, 6,024 sentences, 123,010 words, and 157,593 tokens.
- The saved result API returned that complete result with record id
  `6676c2e8-a7c2-4755-8b19-f51afb76b2ef`; the app's PostgreSQL readiness was healthy.
- The public service produced transient timeouts and one failed page request
  during verification, followed by successful live search and analysis.

## Checks

```sh
rtk uv run pytest tests/test_search.py tests/test_discovery.py tests/test_gutendex_unreachable.py tests/test_store.py -q
rtk cargo test --locked --manifest-path example/Cargo.toml --bin metabook-example
rtk bash example/build-app.sh
rtk git diff --check
```

42 Python tests and 15 native tests passed. Ruff and Mypy passed for the affected
Python code and tests. The app bundle built and launched successfully.
