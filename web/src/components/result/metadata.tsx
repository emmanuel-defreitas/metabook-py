"use client";

import {
  BookOpenIcon,
  ChartBarIcon,
  FingerprintIcon,
  TimerIcon,
  FolderIcon,
  ListChecksIcon,
  ScalesIcon,
} from "@phosphor-icons/react";
import { Badge } from "@/components/ui/badge";
import { isVisibleMetadataValue } from "@/lib/metadata";
import { BarChart, BarChartPlot, BarChartTooltip } from "@/vendor/beui/components/charts/bar-chart";
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

function Section({ title, rows, chart }: { title: string; rows: Row[]; chart?: React.ReactNode }) {
  const visible = rows.filter(([, value]) => isVisibleMetadataValue(value));
  const Icon =
    {
      Bibliographic: BookOpenIcon,
      Classification: FingerprintIcon,
      Record: FolderIcon,
      "Totals & averages": ChartBarIcon,
      Processing: TimerIcon,
      "Detection evidence": ListChecksIcon,
      "Candidate support": ScalesIcon,
    }[title] ?? BookOpenIcon;
  if (!visible.length && !chart) return null;
  return (
    <section className="rounded-xl bg-foreground/3 p-1.5 ring-1 ring-foreground/6 dark:bg-white/3">
      <div className="h-full rounded-xl bg-card px-5 py-4 shadow-[inset_0_1px_1px_rgba(255,255,255,0.6)] dark:shadow-[inset_0_1px_1px_rgba(255,255,255,0.05)]">
        <h3 className="mb-5 flex items-center gap-2.5 font-medium text-xs text-muted-foreground">
          <Icon weight="light" size={20} className="text-foreground/65" />
          {title}
        </h3>
        {chart}
        <dl className="flex flex-col divide-y divide-border/60">
          {visible.map(([label, value]) => (
            <div className="grid grid-cols-[minmax(0,2fr)_minmax(0,3fr)] gap-3 py-2 text-sm" key={label}>
              <dt className="text-muted-foreground">{label}</dt>
              <dd className={value === NOT_PROVIDED ? "text-subtle-foreground" : "wrap-anywhere tabular-nums"}>
                {value}
              </dd>
            </div>
          ))}
        </dl>
      </div>
    </section>
  );
}

function CountChart({
  title,
  entries,
  percent = false,
}: {
  title: string;
  entries: [string, number | null | undefined][];
  percent?: boolean;
}) {
  const values = entries.filter(
    (entry): entry is [string, number] => typeof entry[1] === "number" && Number.isFinite(entry[1]) && entry[1] > 0,
  );
  if (!values.length) return null;
  return (
    <div className="mb-4">
      <BarChart
        categories={values.map(([label]) => label)}
        series={[
          {
            id: "value",
            name: percent ? "Support" : "Count",
            color: "var(--primary)",
            values: values.map(([, value]) => value),
          },
        ]}
        orientation="horizontal"
        label={title}
        max={percent ? 100 : undefined}
        formatValue={(value) => `${value.toLocaleString(undefined, { maximumFractionDigits: 2 })}${percent ? "%" : ""}`}
      >
        <BarChartPlot height={Math.max(100, values.length * 36)} labelWidth={90} />
        <BarChartTooltip />
      </BarChart>
    </div>
  );
}

function SupportGauge({ score }: { score: number }) {
  if (!(score > 0)) return null;
  const percent = Math.round(score * 100);
  return (
    <div className="mb-5 flex items-center gap-6 rounded-2xl bg-foreground/2.5 p-4">
      <svg
        viewBox="0 0 100 100"
        className="size-24 shrink-0 -rotate-90"
        role="img"
        aria-label={`Schema support ${percent}%`}
      >
        <circle
          cx="50"
          cy="50"
          r="40"
          fill="none"
          stroke="currentColor"
          strokeWidth="6"
          className="text-foreground/6"
        />
        <circle
          cx="50"
          cy="50"
          r="40"
          fill="none"
          stroke="currentColor"
          strokeWidth="6"
          strokeLinecap="round"
          pathLength="100"
          strokeDasharray={`${percent} 100`}
          className="text-primary"
        />
      </svg>
      <div>
        <p className="font-mono text-3xl tracking-tight">
          {percent}
          <span className="text-lg text-muted-foreground">%</span>
        </p>
        <p className="mt-1 text-xs text-muted-foreground">Schema support</p>
      </div>
    </div>
  );
}

function SignalBars({ entries, percent = false }: { entries: [string, number][]; percent?: boolean }) {
  const positive = entries.filter(([, value]) => value > 0).sort((a, b) => b[1] - a[1]);
  const max = percent ? 100 : Math.max(...positive.map(([, value]) => value));
  return (
    <div className="space-y-4 pb-2">
      {positive.map(([label, value]) => (
        <div key={label}>
          <div className="mb-2 flex items-baseline justify-between gap-4 text-sm">
            <span className="text-muted-foreground">{label}</span>
            <span className="font-mono text-xs">
              {formatNumber(value, percent ? 1 : 0)}
              {percent ? "%" : ""}
            </span>
          </div>
          <div className="h-1 rounded-lg bg-foreground/5">
            <div
              className="h-full origin-left rounded-lg bg-primary/65"
              style={{ width: `${(value / max) * 100}%` }}
            />
          </div>
        </div>
      ))}
    </div>
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
    ["Subjects", book.subjects?.length ? <div className="flex flex-wrap gap-1.5">{book.subjects.map(subject => <Badge key={subject} variant="secondary" className="h-auto whitespace-normal py-1">{subject}</Badge>)}</div> : null],
    ["ISBN", text(book.isbn)],
    ["Gutenberg ID", isUpload ? "Not applicable · uploaded EPUB" : text(book.gutenberg_id)],
    ["Source", isUpload ? "Uploaded EPUB" : "Project Gutenberg"],
    ["Format", text(record?.format ?? (isUpload ? "epub" : null))],
    ["Publisher", text(book.publisher)],
    ["License", text(book.license)],
    ["Publication date", text(book.date)],
    ["Pages", text(book.number_of_pages)],
    ["Open Library", book.open_library ? <div className="space-y-2">
      {[book.open_library.edition_id, book.open_library.work_id].filter(Boolean).map(id => <a key={id} className="mr-3 underline underline-offset-4" href={`https://openlibrary.org/${id!.endsWith("M") ? "books" : "works"}/${id}`} target="_blank" rel="noreferrer">{id}</a>)}
      <p className="text-xs text-muted-foreground">{book.open_library.match_method === "isbn" ? "Edition matched by ISBN" : "Work matched by title and author; cover may represent another edition"}. Catalog metadata and cover from Open Library.</p>
    </div> : null],
    ["Catalog subjects", book.open_library?.subjects.length ? <div className="flex flex-wrap gap-1.5">{book.open_library.subjects.map(subject => <Badge key={subject} variant="secondary" className="h-auto whitespace-normal py-1">{subject}</Badge>)}</div> : null],
    ["Dewey classification", book.open_library?.dewey_decimal.join(" · ")],
    ["Library of Congress", book.open_library?.lc_classifications.join(" · ")],
  ];

  const storage: Row[] = [
    [
      "State / scope",
      scan ? (scan.scanned ? `Scanned · ${scan.scope ?? "unknown"} detail` : "Not scanned") : NOT_PROVIDED,
    ],
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
    [
      "Mid-level nodes",
      summary.total_mid_level_nodes == null ? "Not applicable" : formatNumber(summary.total_mid_level_nodes),
    ],
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

  return (
    <div className="columns-1 gap-5 lg:columns-2 [&>section]:mb-4 [&>section]:break-inside-avoid [&>div]:mb-4 [&>div]:break-inside-avoid">
      <Section rows={bibliographic} title="Bibliographic" />
      <div className="flex flex-col gap-4">
        <Section rows={classification} title="Classification" chart={<SupportGauge score={structure.schema_score} />} />
        <Section rows={storage} title="Record" />
      </div>
      <Section
        rows={totals}
        title="Totals & averages"
        chart={
          <CountChart
            title="Structure totals"
            entries={[
              ["Paragraphs", summary.total_paragraphs],
              ["Sentences", summary.total_sentences],
              ["Words", summary.total_words],
              ["Tokens", summary.total_tokens],
            ]}
          />
        }
      />
      <div className="flex flex-col gap-4">
        <Section rows={processing} title="Processing" />
        {Object.values(structure.schema_evidence).some((value) => value > 0) && (
          <Section
            rows={[]}
            title="Detection evidence"
            chart={
              <SignalBars
                entries={Object.entries(structure.schema_evidence).map(([key, value]) => [
                  EVIDENCE[key] ?? key.replaceAll("_", " "),
                  value,
                ])}
              />
            }
          />
        )}
        {Object.values(structure.schema_candidates).some((value) => value > 0) && (
          <Section
            rows={[]}
            title="Candidate support"
            chart={
              <SignalBars
                percent
                entries={Object.entries(structure.schema_candidates).map(([key, value]) => [
                  schemaLabel(key),
                  value * 100,
                ])}
              />
            }
          />
        )}
      </div>
    </div>
  );
}
