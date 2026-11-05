// Wire types for the Book Structure API (src/metabook_py/models/*).
// Responses carry positions and counts only, never book text.

export interface Author {
  name: string;
  birth_year?: number | null;
  death_year?: number | null;
}

export interface BookInfo {
  gutenberg_id?: number;
  source?: string;
  title: string;
  authors: Author[];
  language: string;
  subjects: string[];
  isbn?: string | null;
  publisher?: string | null;
  license?: string | null;
  date?: string | null;
  number_of_pages?: number | null;
}

export interface BlobInfo {
  url: string;
  pathname: string;
  size_bytes: number;
}

export interface StructureSummary {
  total_top_level_nodes: number;
  total_mid_level_nodes?: number | null;
  total_paragraphs: number;
  total_sentences: number;
  total_words: number;
  total_tokens?: number;
  avg_paragraphs_per_chapter: number;
  avg_sentences_per_paragraph: number;
  avg_words_per_sentence: number;
}

/** Any node in the tree; shape depends on its depth and the schema. */
export type StructureNode = Record<string, unknown>;

export interface MatterRegion {
  position: "front" | "back";
  kind: string;
  evidence: string;
  start: number;
  end: number;
  word_count: number;
}

export interface StructureDetail {
  matter?: MatterRegion[] | null;
  schema: string;
  schema_confidence: string;
  schema_score: number;
  schema_evidence: Record<string, number>;
  schema_candidates: Record<string, number>;
  schema_detected?: string | null;
  schema_overridden: boolean;
  summary: StructureSummary;
  nodes: StructureNode[];
}

export interface TokenizerInfo {
  name: string;
  vocab_size: number;
}

export interface AnalysisMeta {
  fetched_at?: string;
  uploaded_at?: string;
  cached?: boolean;
  spine_document_count?: number;
  processing_time_ms: number;
  tokenizer?: TokenizerInfo;
}

/** BookStructureResponse | BookUploadResponse. */
export interface Analysis {
  record_id?: string | null;
  book: BookInfo;
  blob?: BlobInfo;
  structure: StructureDetail;
  meta: AnalysisMeta;
}

export interface ScanInfo {
  scanned: boolean;
  last_scanned_at?: string | null;
  scope?: string | null;
  schema?: string | null;
  schema_confidence?: string | null;
  schema_score?: number | null;
  total_tokens?: number | null;
  tokenizer?: string | null;
  summary?: StructureSummary | null;
}

/** One saved book from GET /api/books/uploads. */
export interface UploadRecord {
  id: string;
  source: string;
  gutenberg_id?: number | null;
  book: BookInfo;
  format: string;
  blob?: BlobInfo | null;
  scan: ScanInfo;
  created_at: string;
  updated_at: string;
  size_bytes?: number | null;
}

export type CatalogSource = "gutendex" | "openalex";

export interface CatalogResult {
  source: CatalogSource;
  id: string;
  gutenberg_id?: number | null;
  title: string;
  authors: string[];
  language: string;
  publication_date?: string | null;
  kind: string;
  categories: string[];
  url?: string | null;
}

export interface CatalogPage {
  count: number;
  page: number;
  next_page?: number | null;
  previous_page?: number | null;
  results: CatalogResult[];
  warnings: string[];
}

// ── Single-document analysis (/api/documents/*) ────────────────────────────────

export type DocumentFormat = "html" | "markdown" | "pdf" | "text";

export interface DocumentCounts {
  clauses?: number;
  headings: number;
  paragraphs: number;
  lists: number;
  list_items: number;
  tables: number;
  figures: number;
  code_blocks: number;
  blockquotes: number;
  footnotes: number;
  references: number;
  links: number;
  sentences: number;
  words: number;
}

export interface DocumentTotals extends DocumentCounts {
  sections: number;
  max_depth: number;
}

export interface SectionNode {
  kind: "preamble" | "section";
  level: number;
  index: number;
  path: string;
  counts: DocumentCounts;
  totals: DocumentCounts;
  children: SectionNode[];
  paragraphs?: StructureNode[];
}

export interface DocumentAnalysis {
  document: {
    format: DocumentFormat;
    content_type: string;
    size_bytes: number;
    sha256: string;
    page_count?: number | null;
    filename?: string | null;
    source_url?: string | null;
  };
  structure: { totals: DocumentTotals; sections: SectionNode[] };
  meta: {
    schema_version: string;
    parser: "python" | "rust";
    processing_time_ms: number;
    analyzed_at: string;
  };
}
