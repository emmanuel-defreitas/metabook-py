# Metabook desktop example (GPUI)

An [Ely](https://elygpui.com) native client for the Book Structure API, with the library and book-detail layout from the metaBook Sketch document. The retained JSON editor and lazy structure tree use gpui-component through an Ely theme bridge.

Home is a full-width canvas with padded edges, a borderless window-control row with a top-right Ely light/dark toggle, the metaBook mark, a large Ely `Input` with a right-side search icon, an EPUB `DropZone`, and compact recent saved-book cover previews. Both the search/button gap and the search/drop gap are eight pixels. Search accepts a title or author, an ISBN (including hyphens), or a Gutenberg ID. Browse or drop one EPUB to start analysis immediately.

The Ely theme uses `#F6F6FF` for the light page background and `#18181E` for the dark page background. Other semantic colors and animated theme switching stay managed by Ely.

The supplied green mark is rendered as an Ely image. Light and dark app-icon artwork lives in `assets/branding`; `build-app.sh` packages standard ICNS sizes using AppKit and iconutil. The running macOS Dock icon follows the Ely theme, while Finder uses the light bundle fallback. The original SVG is retained as `logo-source.svg`. Supplied icon images are 258×239 pixels, so larger ICNS sizes are resampled from that artwork.

Search browses [Gutendex](https://gutendex.com) metadata through `/api/books/search`,
including all languages by default. The result count and Previous/Next controls
cover every page (up to 32 books per page). Select a row or **Schema** to analyse
that book and save its structure; browsing results does not download or persist
books. ISBN lookup is a best-effort keyword search because Gutendex does not
provide an ISBN index.

- **Recents** — Compact horizontal cards composed from Ely `Image`, `Icon` and ellipsis typography in a responsive grid for records updated or scanned in the past seven days. Covers are 3.6×5.4 rem (approximately 58×86 pixels); missing covers use a themed book icon. The saved JSON title, authors, actual file size and changed time sit to the right. Click a card or focus it and press Enter/Space to reopen the committed PostgreSQL result. Missing, invalid, or future activity timestamps are excluded.
- **Book detail** — an Ely `NetworkGraph` connects the book to metadata, schema, counts, source information, and structural nodes. Drag and hover nodes; use the named node controls to explore relationships and drill into chapters, paragraphs, sentences, and clauses. Pages show at most 12 children, with Previous/Next covering the entire structure. The tree selection also focuses the graph.
- **Parsing feedback** — Home retains its content while an upload runs: Ely `Banner` and `FileOperationProgress` show actual multipart request bytes, followed by a parsing status while awaiting the committed result. Ely detail skeletons show while a saved book opens or a selected Gutenberg book is analysed. Ely `ResultView` shows success only after a valid structural response arrives; failures show the error and a Return to library action.
- **Structure explorer** — expand the lazy tree and select a node to inspect its JSON fields. **Full JSON** opens the read-only highlighted editor; **Copy JSON** copies the complete API result.

Bibliographic/source metadata, classification, totals, averages, detection evidence,
and candidate scores remain below the graph. Missing fields remain explicit.
The desktop scans at sentence detail with `bert-base-uncased`. Light/dark mode and reduced motion use the shared Ely theme.

The brand or **Library** action returns to the saved books. Book text is never returned in the result.

## Source ownership

- `src/app.rs` owns the request workflow, saved library, window focus, and page composition.
- `src/app/explorer.rs` owns one retained `ResultExplorer` per successful analysis. Its internal modules keep navigation, lazy tree materialization, graph projection, and rendering together.
- `src/app/detail.rs` composes metadata around the explorer's graph and copy action; it does not mutate tree or editor state.
- `src/api.rs` handles blocking requests and response preparation on the background executor.
- `src/theme.rs` bridges Ely's live theme to the retained tree and editor widgets.

Explorer subscriptions and deferred work end when its result closes. Reopening a book starts fresh, and tree selection made while the editor loads is applied when it becomes ready. See [CONTEXT.md](CONTEXT.md) for the desktop domain glossary.

Run focused explorer tests with `cargo test --locked --bin metabook-example app::explorer` after preparing the pinned GPUI checkout below. The entity tests exercise retained state without rendering Ely controls, whose assets are unavailable in `TestAppContext`.

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
