# Metabook desktop example (GPUI)

A small [GPUI](https://www.gpui.rs) / [gpui-component](https://github.com/longbridge/gpui-component) desktop client for the Book Structure API in this repository.

- **Search** — title/author (fuzzy, via Gutendex) or ISBN
- **Upload EPUB** — native file picker, posts to `/api/books/upload`
- **Tokens (optional)** — name a Hugging Face tokenizer (e.g. `bert-base-uncased`) in either form and every node in the tree shows a token count alongside its word count; leave it empty and no tokens are requested

Both paths show a processing view while the API fetches and scans the document, then present the returned structural schema as code (selectable, copyable JSON).

## Run

Start the API from the repository root, then run the app:

```bash
make dev
```

```bash
cd example && cargo run
```

The app targets `http://127.0.0.1:8000` by default (explicit IPv4, so another service listening on `localhost`'s IPv6 side of port 8000 can't shadow the API); point it elsewhere with:

```bash
METABOOK_API=https://your-deployment.example cargo run
```

The first build compiles GPUI from source and takes a while.

## Optional: run as a macOS app bundle

`Metabook.app` is a minimal bundle wrapper (an `Info.plist` plus the app icon in `Contents/Resources/AppIcon.icns`, exported from the project's Sketch logo) so the app has a real bundle identity — useful for macOS permission prompts, Finder launching, and a proper Dock icon. Copy the built binary into it:

```bash
cargo build && cp -f target/debug/metabook-example Metabook.app/Contents/MacOS/ && open Metabook.app
```
