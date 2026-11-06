// Helpers for single-document results (docs/single-document-schema.md).

import type { DocumentAnalysis, DocumentCounts, DocumentFormat, SectionNode } from "./api/types";

export const FORMAT_LABELS: Record<DocumentFormat, string> = {
  html: "HTML",
  markdown: "Markdown",
  pdf: "PDF",
  text: "Plain text",
};

/** Count rows in display order. */
export const COUNT_FIELDS: { key: keyof DocumentCounts; label: string }[] = [
  { key: "headings", label: "Headings" },
  { key: "paragraphs", label: "Paragraphs" },
  { key: "sentences", label: "Sentences" },
  { key: "clauses", label: "Clauses" },
  { key: "words", label: "Words" },
  { key: "lists", label: "Lists" },
  { key: "list_items", label: "List items" },
  { key: "tables", label: "Tables" },
  { key: "figures", label: "Figures" },
  { key: "code_blocks", label: "Code blocks" },
  { key: "blockquotes", label: "Blockquotes" },
  { key: "footnotes", label: "Footnotes" },
  { key: "references", label: "References" },
  { key: "links", label: "Links" },
];

/** Responses carry no heading text, so sections are named by position. */
export function sectionLabel(node: SectionNode): string {
  return node.kind === "preamble" ? "Before the first heading" : `Section ${node.path}`;
}

/** Name for the document: its filename, else the URL's host and path. */
export function documentName(result: DocumentAnalysis): string {
  const { filename, source_url } = result.document;
  if (filename) return filename;
  if (source_url) {
    try {
      const url = new URL(source_url);
      return `${url.host}${url.pathname === "/" ? "" : url.pathname}`;
    } catch {
      return source_url;
    }
  }
  return "Untitled document";
}

/** Every section path, depth first, keyed for lookups. */
export function indexSections(sections: SectionNode[]): Map<string, SectionNode> {
  const index = new Map<string, SectionNode>();
  const walk = (nodes: SectionNode[]) => {
    for (const node of nodes) {
      index.set(node.path, node);
      walk(node.children);
    }
  };
  walk(sections);
  return index;
}

/** Paths to open by default: everything above `depth`. */
export function defaultExpanded(sections: SectionNode[], depth = 1): Set<string> {
  const open = new Set<string>();
  const walk = (nodes: SectionNode[], level: number) => {
    for (const node of nodes) {
      if (node.children.length && level < depth) {
        open.add(node.path);
        walk(node.children, level + 1);
      }
    }
  };
  walk(sections, 0);
  return open;
}

/** Accepts a bare host ("example.com/a") by assuming https. */
export function normalizeUrl(raw: string): string | null {
  const trimmed = raw.trim();
  if (!trimmed) return null;
  const candidate = /^[a-z][a-z\d+.-]*:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`;
  try {
    const url = new URL(candidate);
    return url.protocol === "http:" || url.protocol === "https:" ? url.toString() : null;
  } catch {
    return null;
  }
}

/** Adapt positional document sections to the shared explorer without inventing child data. */
export function documentExplorerAnalysis(result: DocumentAnalysis): import("./api/types").Analysis {
  const convert = (section: SectionNode): import("./api/types").StructureNode => ({
    has_heading: section.kind !== "preamble", total_clauses: section.totals.clauses,
    label: sectionLabel(section), level: "section", index: section.index,
    paragraph_count: section.totals.paragraphs, total_sentences: section.totals.sentences,
    total_words: section.totals.words,
    children: [...(section.paragraphs ?? []), ...section.children.map(convert)],
  });
  const t = result.structure.totals;
  return {
    book: { title: documentName(result), authors: [], language: "", subjects: [] },
    meta: { processing_time_ms: result.meta.processing_time_ms },
    structure: { schema: "document", schema_confidence: "", schema_score: 0,
      schema_evidence: {}, schema_candidates: {}, schema_overridden: false,
      summary: { total_top_level_nodes: result.structure.sections.length, total_paragraphs: t.paragraphs,
        total_sentences: t.sentences, total_words: t.words, avg_paragraphs_per_chapter: 0,
        avg_sentences_per_paragraph: t.paragraphs ? t.sentences / t.paragraphs : 0,
        avg_words_per_sentence: t.sentences ? t.words / t.sentences : 0 },
      nodes: result.structure.sections.map(convert),
    },
  };
}
