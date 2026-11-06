# Single-document analysis schema

Version `1.0`. Applies to `POST /api/documents/analyze`, `POST /api/documents/analyze-url`
and the MCP tool `analyze_document`. All three call one service function and return the
same JSON.

The single-document response carries **counts and positions only, never text**:
no headings, no sentences, no link targets, no alt text. A test walks every response and
fails if any string field outside the metadata allowlist (`filename`, `source_url`,
`content_type`, `sha256`, `analyzed_at`) is longer than 64 characters.

## Inputs

| | `analyze` (multipart) | `analyze-url` (JSON body `{"url": …}`) |
|---|---|---|
| Size limit | 20 MB → `413` | 20 MB, enforced while streaming → `413` |
| Types | see below → `415` | response `Content-Type` (or URL extension when `application/octet-stream`) → `415` |
| Malformed | `422 invalid_document` | `422 invalid_document` |
| URL rules | — | `http`/`https` only; every resolved address must be public (no private, loopback, link-local, CGNAT, reserved or multicast); redirects re-checked hop by hop (max 5) → `400 url_not_allowed` |
| Timeout | — | 10 s total → `504 fetch_timeout`; upstream error → `502 fetch_failed` |

Accepted content types (parameters such as `; charset=` are ignored). When the upload's type
is missing or `application/octet-stream`, the file extension decides.

| Format | Content types | Extensions |
|---|---|---|
| `html` | `text/html`, `application/xhtml+xml` | `.html`, `.htm`, `.xhtml` |
| `markdown` | `text/markdown`, `text/x-markdown` | `.md`, `.markdown` |
| `pdf` | `application/pdf` | `.pdf` |
| `text` | `text/plain` | `.txt`, `.text` |

## Response

```jsonc
{
  "document": {
    "format": "markdown",          // html | markdown | pdf | text
    "content_type": "text/markdown",
    "size_bytes": 1832,
    "sha256": "9f2c…",             // of the raw bytes
    "page_count": null,            // PDF only
    "filename": "paper.md",        // upload only, as sent by the client
    "source_url": null             // analyze-url only
  },
  "structure": {
    "totals": { /* Counts over the whole document */ "sections": 4, "max_depth": 3 },
    "sections": [ /* SectionNode, see below */ ]
  },
  "meta": {
    "schema_version": "1.0",
    "parser": "python",            // python | rust (METABOOK_PARSER)
    "processing_time_ms": 3,
    "analyzed_at": "2026-11-02T05:10:00Z"
  }
}
```

### `SectionNode`

| Field | Type | Meaning |
|---|---|---|
| `kind` | `"preamble"` \| `"section"` | `preamble` holds content before the first heading |
| `level` | int 0–6 | `0` for the preamble, otherwise the heading level (`h1`=1 …) |
| `index` | int ≥ 0 | Ordinal among headed siblings, from 1; the preamble is `0` |
| `path` | string | Dotted ordinal path from the root, e.g. `"2.1.3"`; preamble is `"0"` |
| `counts` | `Counts` | This section's own content, including its heading, excluding subsections |
| `totals` | `Counts` | `counts` plus every descendant section |
| `children` | `SectionNode[]` | Subsections |

### `Counts`

All integers: `headings`, `paragraphs`, `lists`, `list_items`, `tables`, `figures`,
`code_blocks`, `blockquotes`, `footnotes`, `references`, `links`, `sentences`, `words`.
`structure.totals` adds `sections` (all section nodes, preamble included) and `max_depth`
(deepest nesting; a top-level section is depth 1).

## Building the tree

1. A format adapter turns the input into a common block tree. Block types: `heading(level)`,
   `paragraph`, `list`, `list_item`, `table`, `figure`, `code_block`, `blockquote`,
   `footnote`, `reference`, `link`. Containers nest: a `list` holds `list_item`s (which may
   hold nested lists), a `blockquote` holds its own blocks, and `link`s are children of the
   block they appear in.
2. Top-level headings open sections. A heading closes every open section of the same or
   deeper level and nests under the nearest shallower one. **Skipped levels nest directly**:
   `h1 → h3` puts the `h3` (level 3) under the `h1`; a later `h2` closes the `h3` and
   becomes the `h1`'s second child.
3. Content before the first heading goes into a level-0 `preamble` node, created only when
   such content exists. A document with no headings is one preamble. An empty document has
   `sections: []` and zero totals.
4. Headings inside lists or blockquotes are counted in `headings` but do not open sections.

## Counting rules

- **Sentences and words** use the book tokenizer (`services/textstats.py`), so
  numbers match the book endpoints. They are counted for prose blocks only: `paragraph`
  (including paragraphs inside blockquotes), the item's own text in `list_item`, and
  `footnote`. Headings, code, table cells, figure captions and references are not counted.
- **link**: a hyperlink. HTML `<a href>`, Markdown `[text](url)`, `[text][ref]` and
  `<scheme://…>`, plus bare `http(s)://` URLs in Markdown, plain text and PDF.
- **footnote**: a footnote/endnote definition. Markdown `[^id]: …`, HTML `role="doc-footnote"`
  / `doc-endnote`, `id="fn…"`, or class containing `footnote`. In any format, paragraphs
  and list items under a heading named *Notes*, *Footnotes* or *Endnotes* become footnotes.
- **reference**: a bibliography entry. HTML `role="doc-biblioentry"`. In any format,
  paragraphs and list items under a heading named *References*, *Bibliography*,
  *Works Cited*, *Sources* or *Literature Cited* become references.
- **figure**: HTML `<figure>` or an `<img>` outside a figure; a Markdown image alone on its
  line; a plain-text/PDF line starting with `Figure N` / `Fig. N`.

### Format specifics

| Format | Headings | Notes |
|---|---|---|
| html | `h1`–`h6` | `script`, `style`, `template`, `noscript`, `head` skipped; loose text becomes a paragraph |
| markdown | ATX (`#`) and setext (`===` = 1, `---` = 2) | Fenced and indented code, GFM tables, nested lists, front matter skipped |
| text | setext underlines, numbered (`2.` = 1, `2.1` = 2 …), short ALL-CAPS lines = 1 | `>` quotes, `-`/`*`/`•`/`1.` lists, `|`-tables |
| pdf | Lines set larger than the body font; distinct sizes ranked largest = 1 | Paragraphs split on vertical gaps; tables and code are not detected (always 0) |


### Detailed node exploration

The web app requests `?detail=true` on `/api/documents/analyze` and
`/api/documents/analyze-url`. This opt-in uses the Python parser and returns
`paragraphs` for each section's own prose, with sentence, clause and positional
word children. `counts.clauses` and `totals.clauses` report own and cumulative
clause counts. No prose is returned; word nodes have positions only, and no token
counts are fabricated. Existing totals-only requests retain their response shape
and configured parser. Older results must be analysed again to obtain detail.
