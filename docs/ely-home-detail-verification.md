# Ely home and book detail verification

Verified on macOS with the pinned Ely source at `19b0b2997a3e55493e4cd43680c2d80b6397bec6`.
The development bundle runs against its local API and the configured Aiven PostgreSQL database.

## Behavior

- Home has no toolbar, sidebar or master/detail panels. It presents the logo/name, large Ely SearchInput, EPUB DropZone, and RecentFiles/FilePreview metadata cards from the past seven days.
- The pinned library exports the documented drag/drop behavior as `forms::DropZone`; it has no `DragDropFiles` type. Its native picker and external-file target share the same upload callback.
- Uploads show a Banner and FileOperationProgress. Byte counts measure the multipart request consumed by the HTTP client. After transfer, the banner reports parsing while waiting for the API result; the API provides no parser percentage.
- Recents filter recorded created/updated/scan timestamps, sort newest first, and exclude missing, malformed and future timestamps. Their JSON previews contain author/schema/counts, without book prose.
- Book detail uses NetworkGraph with metadata and structural relationships. Named controls navigate because the pinned graph supports hover/drag, but has no node-click callback. Twelve-child pages cover the complete structure; structural navigation synchronizes the lazy tree and JSON selection.
- Ely skeletons appear while saved or selected Gutenberg books open. ResultView displays success after structural response validation, or failure with a Return to library action.

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
