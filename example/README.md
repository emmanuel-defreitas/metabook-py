# Metabook desktop example (GPUI)

An [Ely](https://elygpui.com) native client for the Book Structure API, with the library and book-detail layout from the metaBook Sketch document. The retained JSON editor and lazy structure tree use gpui-component through an Ely theme bridge.

The shared toolbar accepts a title or author, an ISBN (including hyphens), or a Gutenberg ID. Upload EPUB opens the native file picker; select a file and press **Analyze**, or drop an EPUB onto the upload area.

Search browses [Gutendex](https://gutendex.com) metadata through `/api/books/search`,
including all languages by default. The result count and Previous/Next controls
cover every page (up to 32 books per page). Select a row or **Schema** to analyse
that book and save its structure; browsing results does not download or persist
books. ISBN lookup is a best-effort keyword search because Gutendex does not
provide an ISBN index.

- **Library** — compact rows from `/api/books/uploads`, with actual authors, format, structural counts, and scan confidence. Scanned rows reopen their saved PostgreSQL result.
- **Explore** — All books, Theology, Philosophy, and A.I. filter persisted book subjects.
- **Book metadata & structure** — bibliographic and source metadata, schema classification, totals, averages, detection evidence, and candidate scores. Missing fields remain explicit.
- **Structure explorer** — expand the lazy tree and select a node to inspect its JSON fields. **Full JSON** opens the read-only highlighted editor; **Copy JSON** copies the complete API result.
- **Scan options** — detail level and tokenizer, defaulting to sentence detail and `bert-base-uncased`. Light/dark mode uses the shared Ely palette.

The brand or **Library** action returns to the saved books. Book text is never returned in the result.

## Run

Prepare the pinned GPUI source checkout once from the repository root. Cargo's
local patches unify Ely and the retained gpui-component widgets on this revision:

```bash
git clone --filter=blob:none --no-checkout https://github.com/zed-industries/zed example/.gpui
git -C example/.gpui checkout 1a28cff4b409169bac058bca40dfbfeb7621d19b
```

The checkout is ignored by Git. If `example/.gpui` already exists, verify its HEAD
matches that revision before building.

Start the API from the repository root, then run the app:

```bash
make dev
```

```bash
cd example && cargo run
```

The app targets `http://127.0.0.1:8001` by default (explicit IPv4, so another service listening on `localhost`'s IPv6 side of port 8001 can't shadow the API); point it elsewhere with:

```bash
METABOOK_API=https://your-deployment.example cargo run
```

The first build compiles GPUI from source and takes a while.

## Optional: run as a macOS app bundle

Build the development app bundle from the repository root:

```bash
bash example/build-app.sh
open example/Metabook.app
```

The bundle launcher starts the local Python API if port 8001 is not already
healthy, waits for readiness, and stops that API when the app exits. Run `uv sync`
first and configure `DATABASE_URL` and `BLOB_READ_WRITE_TOKEN` in the repository's
ignored `.env.local`. Existing APIs and remote `METABOOK_API` instances are reused.
Logs are in `~/Library/Logs/Metabook/app.log` and `api.log`.

This is a development bundle tied to this checkout and its `.venv`; moving it to
another machine requires that checkout and Python dependencies. Credentials are
not copied into the bundle. The API stores related metadata and structural results
in PostgreSQL, and confirms success only after the transaction commits.
