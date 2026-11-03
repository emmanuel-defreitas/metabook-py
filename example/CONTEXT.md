# Desktop domain glossary

This glossary applies to the Rust GPUI client under `example/`, not the Python
backend.

- **Analysis result**: a validated book response containing metadata and a
  structural schema, plus the derived label tree and JSON node spans. It never
  contains source prose.
- **Result explorer**: the retained module for one analysis result. It owns
  structural tree selection and expansion, graph navigation and pagination,
  the read-only JSON editor, and result-local display/copy feedback.
- **Structure node**: a chapter, paragraph, sentence, clause, word, or other
  schema-specific node. Positional IDs such as `n0.2` identify nodes within one
  immutable analysis result and agree across its tree and JSON spans.
- **Saved library**: persisted book records and shared cover state. Its lifetime
  is independent of the currently open analysis result.
- **Shared filters**: label-free Ely `MultiSelect` controls for date, category and
  type, plus one Source `Select` defaulting to Gutendex. Accessible names remain;
  compact layouts wrap filters beneath the search field. Multi-values combine
  with OR within a dimension, AND across dimensions and the selected source.
  Empty multi-selections are unrestricted, including All dates by default.
  Gutendex is also the default saved-library source; choose Upload for saved EPUBs. Saved date means latest
  saved/scan activity: Today is UTC, other windows roll over 7/30/365 days.
  OpenAlex live dates are inclusive publication-date ranges ending today in UTC.
- **Category**: `subject:<subject>` matches saved subjects exactly; Gutendex maps
  one selected subject to its case-insensitive topic phrase search, not an exact
  upstream subject index. OpenAlex uses verified primary-topic `domain:1..4`
  (Life, Social, Physical and Health Sciences), never guessed from subject labels.
- **Live catalog**: provider-aware metadata from `/api/books/catalog/search`.
  Gutendex and OpenAlex use the same page (up to 32 each); results concatenate
  without deduplication and counts sum. Any provider failure aborts the search.
  All selected providers must support every filter: unsupported dates/categories,
  cross-provider identifiers/types, Upload and schema-only kinds are rejected with
  explanations before provider network I/O. Kindle is not connected.
- **Submitted search**: an owned query/filter snapshot submitted with Enter from
  Home or the toolbar; neither has a Search button. Previous/Next use that snapshot,
  not later UI edits. OpenAlex selection caps paging at 312 with limit warnings.
- **Catalog action**: Gutenberg **Schema** analyses and saves a structure. OpenAlex
  **Open record** opens canonical `https://openalex.org/W…` catalog metadata;
  it does not scan, save or create a schema.
- **Schema support**: the saved heuristic score for automatic schema selection,
  displayed by Ely `PercentText`; it is not calibrated confidence. Missing scores
  remain unknown, not zero. Gutenberg rows can reuse saved scores; OpenAlex
  relevance scores never substitute for schema support.

## Ownership and lifecycle

`MetabookApp` owns requests, library state, window focus, and page composition.
`src/app/catalog.rs` owns catalog submission, pagination and result actions;
`src/api/catalog.rs` owns catalog transport and validated provider metadata.
`src/app/search_filters.rs` owns keys and local matching;
`src/app/components/search_filters.rs` renders the shared Ely `MultiSelect`s.
The shared theme action is a real Ely `IconButton` sized `ControlSize::Lg`, not
an XL circular wrapper. Existing theme and branding/assets ownership is unchanged.

A completed analysis request owns one `Entity<ResultExplorer>`; the shell borrows its
immutable metadata and composes its graph/copy presentation around metadata
sections. The explorer's own `Render` region contains the tree and JSON panes.

Leaving or replacing the result drops that entity's subscriptions and cancels
its pending editor initialization and copy-feedback timer. Reopening a book
creates fresh explorer state. Graph navigation may inspect metadata while
retaining the structural selection shown in the JSON pane.

The module seam intentionally hides mutable tree/editor entities and navigation
state. Internal graph projection, navigation, tree materialization, and rendering
stay together under `src/app/explorer/`. Keep the existing Ely-to-gpui-component
theme adapter and pinned GPUI revision intact.

## Privacy and verification

Query text (`q`) and applicable filters go to the selected external catalogs.
Optional `OPENALEX_API_KEY` is a backend `SecretStr`, sent server-side as a bearer
token, not a desktop credential. Basic keyless access has daily/rate budgets.
No live OpenAlex works search QA has been performed.
