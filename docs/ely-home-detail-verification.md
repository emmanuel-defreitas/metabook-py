# Ely home and book detail verification

Verified on macOS with the pinned Ely source at `19b0b2997a3e55493e4cd43680c2d80b6397bec6`.
The development bundle runs against its local API and the configured Aiven PostgreSQL database.

## Behavior

- Home has no toolbar, sidebar or master/detail panels. It presents the logo/name, large Ely SearchInput, EPUB DropZone, and a responsive grid of FilePreview cover cards from the past seven days.
- The pinned library exports the documented drag/drop behavior as `forms::DropZone`; it has no `DragDropFiles` type. Its native picker and external-file target share the same upload callback.
- Uploads show a Banner and FileOperationProgress. Byte counts measure the multipart request consumed by the HTTP client. After transfer, the banner reports parsing while waiting for the API result; the API provides no parser percentage.
- Recents filter recorded created/updated/scan timestamps, sort newest first, and exclude missing, malformed and future timestamps. Their portrait previews show real cached covers or themed book icons. Captions describe the saved metadata JSON and its actual size, without book prose.
- Book detail uses NetworkGraph with metadata and structural relationships. Named controls navigate because the pinned graph supports hover/drag, but has no node-click callback. Twelve-child pages cover the complete structure; structural navigation synchronizes the lazy tree and JSON selection.
- Ely skeletons appear while saved or selected Gutenberg books open. ResultView displays success after structural response validation, or failure with a Return to library action.

## Markup adjustments — 2026-10-08

- Reduced the home mark from 3.5 rem to 2 rem and the name from 1.25× Display to the theme's Xxl text size.
- Removed the nested search/drop wrappers and their padding. Search and drop now share a zero-gap stack.
- Replaced full-width recent rows and repeated JSON snippets with Ely FilePreview cards in SimpleGrid. Covers and missing-cover icons share a 2:3 ratio; the existing Gutenberg cover cache is reused. Missing-cover SVGs are cached per theme color.
- Cards retain the seven-day filter and saved-result opening, with Tab, Enter/Space, a visible focus ring, and button accessibility labels.
- Native checks at 1505 px and the app's 960 px minimum width showed three correctly sized portrait columns, a loaded Pride and Prejudice cover, and missing-cover icons. Light/dark checks confirmed themed placeholders. Tab from Search through Browse to the first preview, then Enter, reopened its saved structure with HTTP 200.

## Full-width home revision — 2026-10-08

- Removed the separate Ely title bar on home. Detail views retain their controls; a borderless Ely drag region in the home header handles window dragging and double-click zoom.
- Removed the centered 58-rem width cap. Home and recents fill the window with 32-pixel side padding; search and DropZone have an eight-pixel gap.
- Reduced the grid minimum to 12 rem with a one-rem gutter: six cover slots at 1505 pixels, four at the 960-pixel minimum. Existing cover data, portrait placeholders and saved-result activation remain in use.
- Added a top-right Ely IconButton that switches the existing animated Ely theme. Native activation changed the whole home and its placeholders between dark and light.
- All-targets check, formatting, diff check and bundle build passed. The current native suite passed 36 tests and failed the unchanged JSON syntax theme test; a focused rerun reproduced its background-color assertion failure. This layout change does not modify theme or explorer code.

## Automated checks

```sh
cargo test --locked --manifest-path example/Cargo.toml --bin metabook-example
cargo check --locked --all-targets --manifest-path example/Cargo.toml
cargo fmt --manifest-path example/Cargo.toml --check
bash example/build-app.sh
git diff --check
```

All 22 native tests passed. All-target checking, formatting, and the development bundle build passed. Upstream GPUI macOS dependencies still emit deprecation warnings, and `block` reports a future incompatibility warning.
New tests exercise graph pagination and nested navigation, metadata pointer escaping, malformed schema rejection, the seven-day boundary and timestamp offsets, and upload-reader byte counts/EOF.

## Native and database checks

- Opened the rebuilt `example/Metabook.app`; inspected home at 1536 px and 960 px. The search and drop controls fit, recents remain scrollable, and home has no sidebar.
- Home Browse selected the explicit PostgreSQL verification EPUB and started analysis immediately. Captured the parsing banner with a completed 2.4 KB multipart transfer, followed by the success result and graph.
- Upload returned HTTP 201. Queried PostgreSQL directly for record `d60d7358-b32e-4f08-b7c5-040b014c4fb7`: scanned status, standard_book schema, 120 words, and two ordered author relationships.
- Returned home and opened the new recent card. Its saved-structure endpoint returned HTTP 200 with the same record ID.
- Scripted Finder drags did not produce a confirmed external-file drop event. The Ely external-file target is integrated, but actual OS drag remains a manual verification gap; native Browse exercised its shared upload callback.
- Earlier native detail checks exercised all six chapter pages for Pride and Prejudice, graph metadata/structural navigation, loading skeletons, and invalid-EPUB failure feedback.
- Home Search reached the Gutendex endpoint, which returned HTTP 504 during this run. Earlier successful catalog/analysis checks are recorded in `gutendex-search-verification.md`; the external service remains a runtime dependency.

Screenshots captured locally: home, 960 px home, parsing banner/progress, and successful result/graph. Credentials and the application binary remain ignored.
