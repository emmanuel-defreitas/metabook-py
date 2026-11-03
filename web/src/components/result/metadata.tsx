import type { Analysis, UploadRecord } from "@/lib/api/types";
import { authorNames, formatBytes, formatNumber, formatTimestamp, schemaLabel } from "@/lib/format";

const NOT_PROVIDED = "Not provided";

const HIERARCHY: Record<string, string> = {
  canonical_scripture: "Book → Chapter → Verse",
  sectioned_book: "Part → Chapter → Paragraph",
  standard_book: "Chapter → Paragraph",
  essay_or_story_collection: "Essay / Story → Paragraph",
  flat: "Paragraph",
};

const EVIDENCE: Record<string, string> = {
  verse_number_lines: "Verse-number lines",
  part_markers: "Part markers",
  chapter_word_markers: "Chapter-word markers",
  chapter_numeral_markers: "Chapter-numeral markers",
  caps_title_lines: "All-caps title lines",
  title_byline_pairs: "Title / byline pairs",
};

type Row = [label: string, value: React.ReactNode];

const text = (value: unknown): string => {
  if (value == null || value === "") return NOT_PROVIDED;
  if (Array.isArray(value)) return value.length ? value.join("; ") : NOT_PROVIDED;
  return String(value);
};

function Section({ title, rows }: { title: string; rows: Row[] }) {
  return (
    <section className="rounded-[1.75rem] bg-foreground/[0.03] p-1.5 ring-1 ring-foreground/6 dark:bg-white/[0.03]">
      <div className="h-full rounded-[calc(1.75rem-0.375rem)] bg-card px-5 py-4 shadow-[inset_0_1px_1px_rgba(255,255,255,0.6)] dark:shadow-[inset_0_1px_1px_rgba(255,255,255,0.05)]">
        <h3 className="mb-3 font-medium text-[10px] text-muted-foreground uppercase tracking-[0.2em]">
          {title}
        </h3>
        <dl className="flex flex-col divide-y divide-border/60">
          {rows.map(([label, value]) => (
            <div className="grid grid-cols-[minmax(0,2fr)_minmax(0,3fr)] gap-3 py-2 text-sm" key={label}>
              <dt className="text-muted-foreground">{label}</dt>
              <dd
                className={
                  value === NOT_PROVIDED ? "text-subtle-foreground" : "wrap-anywhere tabular-nums"
                }
              >
                {value}
              </dd>
            </div>
          ))}
        </dl>
      </div>
    </section>
  );
}

/** Bibliographic/source metadata, classification, totals, evidence and scores. */
export function MetadataSections({ analysis, record }: { analysis: Analysis; record?: UploadRecord }) {
  const { book, structure, meta, blob } = analysis;
  const summary = structure.summary;
  const isUpload = !!blob || book.source === "upload";
  const scan = record?.scan;

  const bibliographic: Row[] = [
    ["Title", text(book.title)],
    ["Author", authorNames(book.authors)],
    ["Language", text(book.language)],
    ["Subjects", text(book.subjects)],
    ["ISBN", text(book.isbn)],
    ["Gutenberg ID", isUpload ? "Not applicable · uploaded EPUB" : text(book.gutenberg_id)],
    ["Source", isUpload ? "Uploaded EPUB" : "Project Gutenberg"],
    ["Format", text(record?.format ?? (isUpload ? "epub" : null))],
    ["Publisher", text(book.publisher)],
    ["License", text(book.license)],
    ["Publication date", text(book.date)],
    ["Pages", text(book.number_of_pages)],
  ];

  const storage: Row[] = [
    ["State / scope", scan ? (scan.scanned ? `Scanned · ${scan.scope ?? "unknown"} detail` : "Not scanned") : NOT_PROVIDED],
    ["Last scanned", formatTimestamp(scan?.last_scanned_at)],
    ["Created", formatTimestamp(record?.created_at)],
    ["Updated", formatTimestamp(record?.updated_at)],
    ["Uploaded at", isUpload ? formatTimestamp(meta.uploaded_at) : "Not applicable"],
    ["Stored pathname", isUpload ? text(blob?.pathname) : "Not applicable"],
    ["File size", blob ? formatBytes(blob.size_bytes) : "Not applicable"],
  ];

  const classification: Row[] = [
    ["Applied schema", schemaLabel(structure.schema)],
    ["Hierarchy", HIERARCHY[structure.schema] ?? NOT_PROVIDED],
    ["Detected schema", schemaLabel(structure.schema_detected ?? structure.schema)],
    ["Override", structure.schema_overridden ? "Yes · manual selection" : "No · automatic selection"],
    ["Confidence", text(structure.schema_confidence)],
    ["Heuristic support", formatNumber(structure.schema_score * 100, 1) + "%"],
  ];

  const totals: Row[] = [
    ["Top-level nodes", formatNumber(summary.total_top_level_nodes)],
    ["Mid-level nodes", summary.total_mid_level_nodes == null ? "Not applicable" : formatNumber(summary.total_mid_level_nodes)],
    ["Paragraphs", formatNumber(summary.total_paragraphs)],
    ["Sentences", formatNumber(summary.total_sentences)],
    ["Words", formatNumber(summary.total_words)],
    [
      "Tokens / tokenizer",
      summary.total_tokens == null
        ? "Not requested"
        : `${formatNumber(summary.total_tokens)} · ${meta.tokenizer?.name ?? "unknown"}`,
    ],
    ["Paragraphs / chapter", formatNumber(summary.avg_paragraphs_per_chapter, 2)],
    ["Sentences / paragraph", formatNumber(summary.avg_sentences_per_paragraph, 2)],
    ["Words / sentence", formatNumber(summary.avg_words_per_sentence, 2)],
  ];

  const processing: Row[] = [
    ["Processing time", `${formatNumber(meta.processing_time_ms)} ms`],
    ["Vocabulary size", meta.tokenizer ? formatNumber(meta.tokenizer.vocab_size) : "Not requested"],
    ["Spine documents", isUpload ? formatNumber(meta.spine_document_count) : "Not applicable"],
    ["Fetched", meta.fetched_at ? formatTimestamp(meta.fetched_at) : "Not applicable"],
    ["Cached", meta.cached == null ? "Not applicable" : meta.cached ? "Yes" : "No"],
  ];

  const evidence: Row[] = Object.entries(structure.schema_evidence ?? {}).map(([key, value]) => [
    EVIDENCE[key] ?? key,
    formatNumber(value),
  ]);
  const candidates: Row[] = Object.entries(structure.schema_candidates ?? {})
    .sort((a, b) => b[1] - a[1])
    .map(([key, value]) => [schemaLabel(key), `${formatNumber(value * 100, 1)}%`]);

  return (
    <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
      <Section rows={bibliographic} title="Bibliographic" />
      <div className="flex flex-col gap-4">
        <Section rows={classification} title="Classification" />
        <Section rows={storage} title="Record" />
      </div>
      <Section rows={totals} title="Totals & averages" />
      <div className="flex flex-col gap-4">
        <Section rows={processing} title="Processing" />
        {evidence.length > 0 && <Section rows={evidence} title="Detection evidence" />}
        {candidates.length > 0 && <Section rows={candidates} title="Candidate support" />}
      </div>
    </div>
  );
}
