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

## Ownership and lifecycle

`MetabookApp` owns requests, library state, window focus, and page composition.
A completed request owns one `Entity<ResultExplorer>`; the shell borrows its
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
