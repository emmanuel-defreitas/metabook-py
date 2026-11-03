# Metabook web

A Next.js port of the desktop client in [`../example`](../example), served from the
same Vercel domain as the Book Structure API. The web app owns every path except
`/api/*` (plus the `/mcp` and `/health` aliases), which Vercel routes to the
FastAPI service. See [`../vercel.json`](../vercel.json).

## Stack

- Next.js 16 (App Router, Cache Components) and React 19
- [shadcn/ui](https://ui.shadcn.com) CLI with the [coss UI](https://coss.com/ui)
  registry (`@coss/*`, Base UI primitives) in `src/components/ui`
- Tailwind CSS 4, with colors taken from the desktop's exported semantic tokens
  (`example/assets/design-tokens.json`)
- TanStack Query for API data, `next-themes` for light/dark, Phosphor icons
  (light weight), Motion for reveals

Add more coss components with `npx shadcn@latest add @coss/<name>`. Browse
copy-paste patterns at <https://coss.com/ui/particles>.

## Features (desktop parity)

| Desktop | Web |
|---|---|
| Home search (Enter submits) with Date / Category / Type / Source filters | `/` hero search; the same filters also narrow the saved library as you type |
| EPUB drop zone with byte-level upload progress | `/` drop zone (XHR upload progress, then a parsing state) |
| Recents with covers, size, changed time and schema support | `/#library` grid |
| Catalog results, Previous/Next over the submitted snapshot | `/search?q=…&source=…&page=…` (the URL is the snapshot) |
| Gutenberg **Schema** analyses and saves | `/gutenberg/[id]`, then redirects to `/books/[record_id]` |
| OpenAlex **Open record** | Opens `https://openalex.org/W…` in a new tab |
| NetworkGraph with 12-per-page drill-down | Radial structure map (double-click or Enter to open a node) |
| Lazy structure tree + JSON editor, Full JSON, Copy JSON | Tree, node JSON pane, Full JSON dialog, copy and download |
| Metadata, totals, evidence and candidate scores | Metadata sections below the explorer |

Scans use `detail=sentence` and `tokenizer=bert-base-uncased`, like the desktop.

## Develop

Start the API from the repository root (`make dev`, port 8001), then:

```bash
cd web
npm install
npm run dev
```

In development, `/api/*` is proxied to `http://127.0.0.1:8001`. Point it at
another API with `METABOOK_API_URL=https://… npm run dev`. To call an API on a
different origin from the browser instead, set `NEXT_PUBLIC_API_BASE`.

```bash
npm run lint
npm run build
```

## API endpoints used

- `GET /api/books/catalog/search`: live Gutendex / OpenAlex metadata
- `GET /api/books/uploads`: the saved library
- `GET /api/books/uploads/{id}/structure`: reopen a saved result
- `GET /api/books/structure?gutenberg_id=…`: analyse and save a Gutenberg book
- `POST /api/books/upload`: analyse an EPUB

The first three ship with the desktop's backend work (`feat/gpui-update`). Until
that reaches `dev`, the web app shows a clear "endpoint not available" message
for catalog search and saved results.
