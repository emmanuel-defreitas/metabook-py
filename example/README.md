# Metabook desktop example (GPUI)

A small [GPUI](https://www.gpui.rs) / [gpui-component](https://github.com/longbridge/gpui-component) desktop client for the Book Structure API in this repository.

The window is a sidebar workspace: the sidebar (collapsible to an icon rail) holds the app identity and the **Dashboard** destination, and the work area is the Dashboard until a scan finishes.

- **Search** — title/author (fuzzy, via Gutendex) or ISBN
- **Upload EPUB** — drag a file onto the drop zone, or pick one with the native file picker; either way it posts to `/api/books/upload`
- **Explore** — a cover grid of every book the API has persisted (`/api/books/uploads`); Gutenberg books show their cover and re-scan when clicked, uploaded EPUBs show a placeholder because their file lives in private blob storage
- **Tokens** — pick a Hugging Face tokenizer from the dropdown (default `bert-base-uncased`) and every node in the tree shows a token count alongside its word count; choose “No tokens” and none are requested

Any path shows a processing view while the API fetches and scans the document, then presents the returned structural schema as code (selectable, copyable JSON). The sidebar's Dashboard item takes you back.

## Run

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

`Metabook.app` is a minimal bundle wrapper (an `Info.plist` plus the app icon in `Contents/Resources/AppIcon.icns`, exported from the project's Sketch logo) so the app has a real bundle identity — useful for macOS permission prompts, Finder launching, and a proper Dock icon. Copy the built binary into it:

```bash
cargo build && cp -f target/debug/metabook-example Metabook.app/Contents/MacOS/ && open Metabook.app
```
