"use client";

import { ArrowLeftIcon } from "@phosphor-icons/react";
import Link from "next/link";
import { useState } from "react";
import { SchemaScore } from "@/components/library/score";
import { Reveal } from "@/components/motion/reveal";
import { Badge } from "@/components/ui/badge";
import type { Analysis, UploadRecord } from "@/lib/api/types";
import { authorNames, formatNumber, schemaLabel } from "@/lib/format";
import { JsonPane } from "./json-pane";
import { MetadataSections } from "./metadata";
import { StructureMap } from "./structure-map";
import { StructureTree } from "./structure-tree";

function Shell({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={`rounded-[2rem] bg-foreground/[0.03] p-1.5 ring-1 ring-foreground/6 dark:bg-white/[0.03] ${className ?? ""}`}>
      <div className="h-full rounded-[calc(2rem-0.375rem)] bg-card p-4 shadow-[inset_0_1px_1px_rgba(255,255,255,0.6)] sm:p-5 dark:shadow-[inset_0_1px_1px_rgba(255,255,255,0.05)]">
        {children}
      </div>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col gap-1 rounded-2xl px-4 py-3 ring-1 ring-foreground/6">
      <span className="text-[10px] text-muted-foreground uppercase tracking-[0.18em]">{label}</span>
      <span className="font-semibold text-2xl tabular-nums tracking-tight">{value}</span>
    </div>
  );
}

export function ResultView({ analysis, record }: { analysis: Analysis; record?: UploadRecord }) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [focusId, setFocusId] = useState<string | null>(null);
  const { book, structure } = analysis;
  const summary = structure.summary;

  // Selecting in the tree focuses the map on that node's parent level.
  const selectFromTree = (id: string) => {
    setSelectedId(id);
    setFocusId(id.includes(".") ? id.slice(0, id.lastIndexOf(".")) : null);
  };

  return (
    <div className="mx-auto flex w-full max-w-6xl flex-col gap-10 px-4 pt-10 pb-32 sm:px-8 sm:pt-14">
      <Reveal className="flex flex-col gap-6">
        <Link
          className="flex w-max items-center gap-2 rounded-full px-1 text-muted-foreground text-sm outline-none transition-colors hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
          href="/#library"
        >
          <ArrowLeftIcon weight="light" />
          Library
        </Link>
        <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
          <div className="flex max-w-3xl flex-col gap-3">
            <div className="flex flex-wrap gap-1.5">
              <Badge variant="outline">{schemaLabel(structure.schema)}</Badge>
              <Badge variant={structure.schema_confidence === "high" ? "success" : structure.schema_confidence === "low" ? "warning" : "info"}>
                {structure.schema_confidence} confidence
              </Badge>
              {book.gutenberg_id ? <Badge variant="secondary">Gutenberg #{book.gutenberg_id}</Badge> : <Badge variant="secondary">Uploaded EPUB</Badge>}
            </div>
            <h1 className="text-balance font-semibold text-4xl leading-[1.05] tracking-[-0.03em] sm:text-5xl">{book.title}</h1>
            <p className="text-lg text-muted-foreground">{authorNames(book.authors)}</p>
          </div>
          <div className="w-full max-w-60">
            <SchemaScore score={structure.schema_score} />
          </div>
        </div>
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
          <Stat label="Paragraphs" value={formatNumber(summary.total_paragraphs)} />
          <Stat label="Sentences" value={formatNumber(summary.total_sentences)} />
          <Stat label="Words" value={formatNumber(summary.total_words)} />
          <Stat label="Tokens" value={summary.total_tokens == null ? "—" : formatNumber(summary.total_tokens)} />
        </div>
      </Reveal>

      <Reveal className="grid grid-cols-1 gap-4 lg:grid-cols-12" delay={0.08}>
        <Shell className="lg:col-span-7">
          <StructureMap
            analysis={analysis}
            focusId={focusId}
            onFocus={(id) => {
              setFocusId(id);
              if (id) setSelectedId(id);
            }}
            onSelect={setSelectedId}
            selectedId={selectedId}
          />
        </Shell>
        <Shell className="lg:col-span-5">
          <JsonPane analysis={analysis} selectedId={selectedId} />
        </Shell>
      </Reveal>

      <Reveal delay={0.1}>
        <Shell>
          <div className="mb-3 flex items-baseline justify-between gap-2">
            <h2 className="font-medium tracking-tight">Structure explorer</h2>
            <span className="text-subtle-foreground text-xs">{formatNumber(summary.total_top_level_nodes)} top-level nodes</span>
          </div>
          <StructureTree onSelect={selectFromTree} selectedId={selectedId} structure={structure} />
        </Shell>
      </Reveal>

      <Reveal delay={0.1}>
        <MetadataSections analysis={analysis} record={record} />
      </Reveal>
    </div>
  );
}
