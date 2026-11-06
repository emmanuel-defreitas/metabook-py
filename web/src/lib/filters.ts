// Shared catalog filter keys and saved-library matching (port of the desktop's
// search_filters.rs). OR within a dimension, AND across dimensions.

import type { UploadRecord } from "./api/types";

export interface Option {
  value: string;
  label: string;
  note?: string;
  disabled?: boolean;
}

export const DATE_OPTIONS: Option[] = [
  { value: "today", label: "Today (UTC)" },
  { value: "7days", label: "Past 7 days" },
  { value: "30days", label: "Past 30 days" },
  { value: "year", label: "Past year" },
];

export const KIND_OPTIONS: Option[] = [
  { value: "book", label: "Book" },
  { value: "academic", label: "Academic" },
  { value: "canon", label: "Canon", note: "Saved library only" },
  { value: "corpus", label: "Corpus", note: "Saved library only" },
  { value: "collection", label: "Collection", note: "Saved library only" },
  { value: "other", label: "Other" },
];

export const SOURCE_OPTIONS: Option[] = [
  { value: "gutendex", label: "Gutendex" },
  { value: "openalex", label: "OpenAlex" },
  { value: "upload", label: "Upload", note: "Saved books and documents" },
  { value: "kindle", label: "Kindle", note: "Not connected yet", disabled: true },
];

/** OpenAlex primary-topic domains (api.openalex.org/domains). */
export const DOMAIN_OPTIONS: Option[] = [
  { value: "domain:1", label: "Life Sciences" },
  { value: "domain:2", label: "Social Sciences" },
  { value: "domain:3", label: "Physical Sciences" },
  { value: "domain:4", label: "Health Sciences" },
];

export interface SearchFilters {
  date: string[];
  category: string[];
  kind: string[];
  source: string;
}

export const DEFAULT_FILTERS: SearchFilters = {
  date: [],
  category: [],
  kind: [],
  source: "gutendex",
};

/** Domains first, then exact saved subjects (plus any already selected). */
export function categoryOptions(records: UploadRecord[], selected: string[]): Option[] {
  const subjects = new Set<string>();
  for (const record of records) {
    for (const subject of record.book?.subjects ?? []) if (subject) subjects.add(subject);
  }
  for (const value of selected) {
    if (value.startsWith("subject:") && value.length > 8) subjects.add(value.slice(8));
  }
  return [
    ...DOMAIN_OPTIONS,
    ...[...subjects].sort().map((subject) => ({
      value: `subject:${subject}`,
      label: subject,
      note: "Saved subject / Gutendex topic",
    })),
  ];
}

export function filtersToParams(filters: SearchFilters, params = new URLSearchParams()) {
  for (const value of filters.date) params.append("date", value);
  for (const value of filters.category) params.append("category", value);
  for (const value of filters.kind) params.append("kind", value);
  if (filters.source) params.set("source", filters.source);
  return params;
}

export function filtersFromParams(params: URLSearchParams): SearchFilters {
  return {
    date: params.getAll("date"),
    category: params.getAll("category"),
    kind: params.getAll("kind"),
    source: params.get("source") ?? DEFAULT_FILTERS.source,
  };
}

// ── Saved-library matching ─────────────────────────────────────────────────────

const DAY_MS = 24 * 60 * 60 * 1000;

type Loose = Record<string, unknown> & UploadRecord;

/** Latest saved/scan activity; a future timestamp is rejected, not skipped. */
export function recordedActivity(record: UploadRecord, now = Date.now()): number | null {
  const times = [record.updated_at, record.created_at, record.scan?.last_scanned_at]
    .map((value) => (value ? Date.parse(value) : Number.NaN))
    .filter((value) => !Number.isNaN(value));
  if (!times.length) return null;
  const latest = Math.max(...times);
  return latest <= now ? latest : null;
}

function sourceKey(source: unknown): string | null {
  switch (String(source ?? "").trim().toLowerCase()) {
    case "gutenberg":
    case "gutendex":
      return "gutendex";
    case "document":
    case "upload":
      return "upload";
    case "openalex":
      return "openalex";
    case "kindle":
      return "kindle";
    default:
      return null;
  }
}

const KINDS = new Set(KIND_OPTIONS.map((option) => option.value));

export function recordKind(record: UploadRecord): string {
  const loose = record as Loose;
  const book = record.book as unknown as Record<string, unknown>;
  const explicit = [loose.type, book?.type].filter((v): v is string => typeof v === "string");
  const known = explicit.map((v) => v.trim().toLowerCase()).find((v) => KINDS.has(v));
  if (known) return known;
  const source = sourceKey(record.source);
  if (!explicit.some((v) => v.trim()) && source === "openalex") return "academic";
  const schema = record.scan?.schema ?? (loose.schema as string | undefined);
  switch (schema) {
    case "canonical_scripture":
      return "canon";
    case "essay_or_story_collection":
      return "collection";
    case "standard_book":
    case "sectioned_book":
    case "flat":
      return "book";
    case undefined:
    case null:
      return source === "gutendex" || source === "upload" ? "book" : "other";
    default:
      return "other";
  }
}

function matchesDate(record: UploadRecord, dates: string[], now: number): boolean {
  if (!dates.length) return true;
  const activity = recordedActivity(record, now);
  if (activity === null) return false;
  return dates.some((date) => {
    if (date === "today") return Math.floor(activity / DAY_MS) === Math.floor(now / DAY_MS);
    const days = { "7days": 7, "30days": 30, year: 365 }[date];
    return days !== undefined && now - activity <= days * DAY_MS;
  });
}

function matchesCategory(record: UploadRecord, category: string): boolean {
  if (category.startsWith("subject:")) {
    const subject = category.slice(8);
    return !!subject && (record.book?.subjects ?? []).includes(subject);
  }
  if (!category.startsWith("domain:")) return false;
  const loose = record as Loose;
  const topic = (loose.primary_topic ?? (record.book as unknown as Record<string, unknown>)?.primary_topic) as
    | { domain?: { id?: unknown } }
    | undefined;
  const id = String(topic?.domain?.id ?? "").replace("https://openalex.org/domains/", "");
  return id === category.slice(7);
}

const normalize = (text: string) => text.split(/\s+/).filter(Boolean).join(" ").toLowerCase();
const normalizeIsbn = (text: string) => text.replace(/[\s-]/g, "").toLowerCase();

function matchesQuery(record: UploadRecord, raw: string): boolean {
  const query = normalize(raw);
  if (!query) return true;
  const book = record.book ?? ({} as UploadRecord["book"]);
  const has = (value: unknown) => typeof value === "string" && normalize(value).includes(query);
  if (has(book.title)) return true;
  if (book.isbn && normalizeIsbn(query) && normalizeIsbn(book.isbn).includes(normalizeIsbn(query)))
    return true;
  if ((book.authors ?? []).some((author) => has(author) || has(author?.name))) return true;
  if ((book.subjects ?? []).some(has)) return true;
  const id = query.replace(/^#/, "").trim();
  return !!id && record.gutenberg_id != null && String(record.gutenberg_id).includes(id);
}

export function matchesRecord(
  record: UploadRecord,
  filters: SearchFilters,
  query: string,
  now = Date.now(),
): boolean {
  return (
    matchesDate(record, filters.date, now) &&
    (!filters.category.length || filters.category.some((c) => matchesCategory(record, c))) &&
    (!filters.kind.length || filters.kind.includes(recordKind(record))) &&
    (!filters.source || sourceKey(filters.source) === sourceKey(record.source)) &&
    matchesQuery(record, query)
  );
}
