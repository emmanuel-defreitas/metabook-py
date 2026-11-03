# Metabook desktop example (GPUI)

An [Ely](https://elygpui.com) native client for the Book Structure API, with the library and book-detail layout from the metaBook Sketch document. The retained JSON editor and lazy structure tree use gpui-component through an Ely theme bridge.

Home is a full-width canvas with padded edges, an Ely `IconButton` sized `ControlSize::Lg` in the top-right window chrome, the metaBook mark, a large Ely `Input`, and an EPUB `DropZone` on a muted surface. Home and the detail/search toolbar share label-free Ely `MultiSelect` controls for Date, Category and Type, plus an Ely single `Select` for Source with Gutendex selected by default. Accessible names are retained. In compact windows the filters wrap below the search field, then wrap within their own row as needed. Press Enter in either search field to submit; neither has a Search button. Browse or drop one EPUB to start analysis immediately.

The Ely theme uses the supplied DesignSystems.one semantic palette: `#F9FAFD` for the light page background and `#0F1217` for the dark page background. Animated theme switching stays managed by Ely.

The supplied green mark is rendered as an Ely image. The macOS icon source is `assets/branding/metabook.icon`, preserved from Icon Composer. `build-app.sh` uses Xcode’s `actool` to compile its layered artwork and appearance variants into `Assets.car`, plus an ICNS fallback for older macOS versions. macOS controls the icon appearance; the app’s Ely theme toggle controls the app content. The original logo SVG is retained as `logo-source.svg`.

Live metadata search uses `/api/books/catalog/search` with one selected provider: [Gutendex](https://gutendex.com) by default, or [OpenAlex](https://openalex.org). Empty Date, Category and Type selections are unrestricted. Gutendex searches titles and author names; OpenAlex searches work keywords (titles, abstracts and available full text), not author identity. ISBNs (including hyphens) and Gutenberg IDs are Gutendex-only; ISBN lookup remains a best-effort keyword search, not an ISBN index. Browsing never downloads book text or saves results. A Gutenberg row or **Schema** analyses and saves the book; an OpenAlex row or **Open record** opens its canonical HTTPS catalog page, without schema analysis, scanning or saving. OpenAlex relevance is never substituted for schema support.

The selected provider returns up to 32 results per page. The backend also supports multi-source requests, querying the same page from each provider without deduplication; this is not exposed by the desktop's single-source selector. Selecting OpenAlex caps pagination at page 312, with warnings for remaining matches. Previous/Next retain the submitted query/filter snapshot: edits apply only on the next Enter submission.

- **Shared filters** — OR within Date, Category and Type, AND across dimensions and the single selected Source. Saved metadata matches as you type, using exact subjects, declared types or known schemas. The default Gutendex source excludes uploads; select Upload to show saved EPUBs. All dates is now the saved-library default, not the past seven days. Today uses UTC; other saved windows roll over latest saved/scan activity. Live OpenAlex dates instead use inclusive publication-date ranges ending today (UTC), with 7/30/365 calendar days.
- **Provider limits** — Categories distinguish exact saved `subject:<subject>` values from verified OpenAlex primary-topic domains: `domain:1` Life Sciences, `2` Social Sciences, `3` Physical Sciences, `4` Health Sciences. Gutendex maps one subject to a case-insensitive topic phrase search, not exact upstream matching; multiple subjects are unsupported remotely. Gutendex has no publication-date/domain filters and supports only unrestricted or Book type. OpenAlex supports domains and Book/Academic/exact Other types, not Gutendex subjects or ISBN/Gutenberg IDs. Canon/Corpus/Collection and Upload are saved-only; Kindle is disabled and not connected. Unsupported or cross-provider combinations return explicit explanations before any provider request, never silently ignored filters.
- **Recents** — Responsive saved-book cards use a shared 2:3 portrait frame at half the card width, with a closed Book icon when no cover exists. Metadata includes the saved JSON filename, authors, actual serialized size and changed time. Ely `PercentText` shows the stored schema support score; missing scores remain explicit rather than showing zero. Scores are heuristic support, not calibrated confidence. Gutendex result rows reuse a saved score when available. Click a card or focus it and press Enter/Space to reopen the committed PostgreSQL result. Bounded date filters exclude missing, invalid or future activity timestamps; All dates includes undated records with an unavailable-date label.
- **Book detail** — an Ely `NetworkGraph` connects the book to metadata, schema, counts, source information, and structural nodes. Drag and hover nodes; use the named node controls to explore relationships and drill into chapters, paragraphs, sentences, and clauses. Pages show at most 12 children, with Previous/Next covering the entire structure. The tree selection also focuses the graph.
- **Parsing feedback** — Home retains its content while an upload runs: Ely `Banner` and `FileOperationProgress` show actual multipart request bytes, followed by a parsing status while awaiting the committed result. Ely detail skeletons show while a saved book opens or a selected Gutenberg book is analysed. Ely `ResultView` shows success only after a valid structural response arrives; failures show the error and a Return to library action.
- **Structure explorer** — expand the lazy tree and select a node to inspect its JSON fields. **Full JSON** opens the read-only highlighted editor; **Copy JSON** copies the complete API result.

Bibliographic/source metadata, classification, totals, averages, detection evidence,
and candidate scores remain below the graph. Missing fields remain explicit.
The desktop scans at sentence detail with `bert-base-uncased`. Light/dark mode and reduced motion use the shared Ely theme.

The brand or **Library** action returns to the saved books. Book text is never returned in the result.

Catalog query text (`q`) and applicable filters are shared with the selected external catalogs; avoid sensitive queries. Basic keyless OpenAlex access has daily/rate budgets. Optional `OPENALEX_API_KEY` belongs in the backend environment (`.env.local` locally): it is held as `SecretStr` and sent server-side as a bearer token, never embedded in the desktop app. No live OpenAlex works search QA has been performed.

## Source ownership

- `src/app.rs` owns the request workflow, saved library, window focus, and page composition.
- `src/app/explorer.rs` owns one retained `ResultExplorer` per successful analysis. Its internal modules keep navigation, lazy tree materialization, graph projection, and rendering together.
- `src/app/detail.rs` composes metadata around the explorer's graph and copy action; it does not mutate tree or editor state.
- `src/app/catalog.rs` owns catalog submission, snapshot pagination and provider-specific result actions.
- `src/app/search_filters.rs` owns filter keys, saved-library matching and metadata classification. `src/app/components/search_filters.rs` composes the three Ely `MultiSelect` controls and single Source `Select`; the other components own the `Lg` theme `IconButton`, percentage scores and book previews.
- `src/api.rs` handles blocking requests and response preparation on the background executor; `src/api/catalog.rs` owns catalog transport, validated provider identities and submitted snapshots.
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

Color tokens are checked in at `assets/design-tokens.json`, from the supplied DesignSystems.one export. `src/theme/tokens.rs` maps its light/dark semantic colors to Ely, including primary actions, neutral surfaces, feedback, focus, charts and code syntax. Typography, spacing, radius and motion remain governed by Ely’s existing settings.
