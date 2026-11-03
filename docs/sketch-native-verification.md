# Metabook native Sketch verification

Verified 7 October 2026 against the saved metaBook Sketch document's Library and Book detail frames in Ely dark and light themes.

## Implemented

- Shared search toolbar accepting title/author, ISBN, or Gutenberg ID; upload and theme actions.
- 240 px sidebar with Ely navigation and subject filters; compact persisted-library rows with covers, author/language/format, counts, and actual scan confidence.
- Bibliographic metadata, source/scan metadata, schema classification, automatic detection, cleaned-body totals, averages/tokenizer, evidence, and candidate support in paired columns.
- Lazy structure tree, selected node scalar JSON fields, optional full highlighted JSON, and copy feedback.
- Existing Ely theme bridge for retained input/select/tree/editor widgets, plus Ely root keyboard focus scope for Tab navigation. macOS traffic lights remain in their native title strip above the shared toolbar.

## Checks

```sh
rtk cargo test --locked --manifest-path example/Cargo.toml --bin metabook-example
rtk bash example/build-app.sh
rtk git diff --check
```

13 native tests passed, including unified identifier parsing, nested selected-node fields, subject filters, chapter summary counts, existing API/tree formatting, and theme mapping.

Manual native verification covered library navigation, stored result reopening after restart, both themes, 1000 px window wrapping, scan options, chapter/paragraph expansion, selected fields, full JSON, and valid copied JSON.

Upload verification used the clearly named `Metabook PostgreSQL verification` EPUB fixture through the native file picker and Analyze action. Its newly saved record is `9a2bad7a-616f-4ffc-86c2-ea213dea5c1e`. A direct TLS PostgreSQL query joined books, metadata, and schemas and confirmed scanned status, 42 pages, standard_book, 150 tokens, 0.6 heuristic score, and an object-valued saved result. No extracted book prose appears in the API result. The library contains the earlier verification record and this additional upload verification record.

## Screenshots

Screenshots are saved at:

`/Users/emmanuel/.codex/visualizations/2026/10/07/01a118c4-7e7b-7ec0-a2fc-3ed7d300dc1b/metabook-native-sketch/`

Files: library-dark.png, library-light.png, detail-dark.png, detail-light.png, structure-dark.png.

Sketch sample book values are not seeded into the app; every displayed book row and detail value comes from the persisted library and API result. The app bundle uses the repository API launcher and ignored local environment configuration. No credentials are embedded in the native client.
