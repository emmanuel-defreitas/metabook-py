// Structure → label tree (port of the desktop's build_tree). Ids are
// positional ("n0", "n0.2", "n0.2.4") and identify a node within one result.
// Children are derived lazily so very deep results stay cheap to open.

import type { StructureDetail, StructureNode } from "./api/types";

export type NodeKind = "section" | "chapter" | "paragraph" | "sentence" | "clause" | "word";

export interface TreeNode {
  id: string;
  label: string;
  meta: string;
  kind: NodeKind;
  raw: StructureNode;
  childCount: number;
}

const CHILD_KEYS = ["children", "paragraphs", "sentences", "clauses", "words"] as const;

const num = (value: unknown) => (typeof value === "number" ? value : 0);

export function plural(n: number, noun: string): string {
  return `${n.toLocaleString()} ${noun}${n === 1 ? "" : "s"}`;
}

const tokens = (node: StructureNode, key = "token_count") =>
  typeof node[key] === "number" ? ` · ${plural(node[key] as number, "token")}` : "";

function childKey(node: StructureNode): (typeof CHILD_KEYS)[number] | null {
  return CHILD_KEYS.find((key) => Array.isArray(node[key])) ?? null;
}

function childArray(node: StructureNode): StructureNode[] {
  const key = childKey(node);
  return key ? (node[key] as StructureNode[]) : [];
}

function nodeLabel(node: StructureNode, fallback: string): string {
  const label = typeof node.label === "string" ? node.label.trim() : "";
  if (label) return label;
  const level = typeof node.level === "string" ? node.level : fallback;
  return `${level[0].toUpperCase()}${level.slice(1)} ${num(node.index)}`;
}

/** Classify a node by its shape, given the parent kind. */
function describe(node: StructureNode, id: string, parent: NodeKind | null, leaf: string): TreeNode {
  const childCount = childArray(node).length;
  const base = { id, raw: node, childCount };
  if (Array.isArray(node.children)) {
    return { ...base, kind: "section", label: nodeLabel(node, "Section"), meta: plural(num(node.child_count) || childCount, "chapter") + ` · ${plural(num(node.total_words), "word")}${tokens(node, "total_tokens")}` };
  }
  if (typeof node.paragraph_count === "number") {
    return { ...base, kind: "chapter", label: nodeLabel(node, "Chapter"), meta: `${plural(num(node.paragraph_count), "paragraph")} · ${plural(num(node.total_words), "word")}${tokens(node, "total_tokens")}` };
  }
  if (typeof node.sentence_count === "number") {
    return { ...base, kind: "paragraph", label: `${leaf} ${num(node.index)}`, meta: `${plural(num(node.sentence_count), "sentence")} · ${plural(num(node.word_count), "word")}${tokens(node)}` };
  }
  if (typeof node.clause_count === "number") {
    return { ...base, kind: "sentence", label: `Sentence ${num(node.index)}`, meta: `${plural(num(node.clause_count), "clause")} · ${plural(num(node.word_count), "word")}${tokens(node)}` };
  }
  if (typeof node.word_count === "number" || parent === "sentence") {
    return { ...base, kind: "clause", label: `Clause ${num(node.index)}`, meta: `${plural(num(node.word_count), "word")}${tokens(node)}` };
  }
  return { ...base, kind: "word", label: `Word ${num(node.index)}`, meta: "" };
}

export function leafName(structure: StructureDetail): string {
  return structure.schema === "canonical_scripture" ? "Verse" : "Paragraph";
}

export function rootNodes(structure: StructureDetail): TreeNode[] {
  const leaf = leafName(structure);
  return structure.nodes.map((node, ix) => describe(node, `n${ix}`, null, leaf));
}

export function childNodes(parent: TreeNode, leaf: string): TreeNode[] {
  return childArray(parent.raw).map((node, ix) => describe(node, `${parent.id}.${ix}`, parent.kind, leaf));
}

/** Resolve a positional id back to its node. */
export function findNode(structure: StructureDetail, id: string): TreeNode | null {
  const parts = id.replace(/^n/, "").split(".").map(Number);
  if (parts.some(Number.isNaN)) return null;
  const leaf = leafName(structure);
  const first = structure.nodes[parts[0]];
  if (!first) return null;
  let current = describe(first, `n${parts[0]}`, null, leaf);
  for (const ix of parts.slice(1)) {
    const next = childArray(current.raw)[ix];
    if (!next) return null;
    current = describe(next, `${current.id}.${ix}`, current.kind, leaf);
  }
  return current;
}

/** Ids of every ancestor of `id`, root first, excluding `id` itself. */
export function ancestorIds(id: string): string[] {
  const parts = id.split(".");
  return parts.slice(0, -1).map((_, ix) => parts.slice(0, ix + 1).join("."));
}

/** A node's own fields, with nested children summarised instead of inlined. */
export function nodeFields(node: StructureNode): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(node)) {
    if ((CHILD_KEYS as readonly string[]).includes(key) && Array.isArray(value)) {
      out[key] = `[${plural(value.length, "item")}]`;
    } else {
      out[key] = value;
    }
  }
  return out;
}
