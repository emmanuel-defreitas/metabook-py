"use client";

import { ArrowCounterClockwiseIcon, CopyIcon, DownloadSimpleIcon } from "@phosphor-icons/react";
import { useMemo } from "react";
import { Reveal } from "@/components/motion/reveal";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { toastManager } from "@/components/ui/toast";
import type { DocumentAnalysis, DocumentCounts } from "@/lib/api/types";
import { COUNT_FIELDS, documentName, FORMAT_LABELS, indexSections, sectionLabel } from "@/lib/document";
import { formatBytes, formatNumber, formatTimestamp } from "@/lib/format";
import { plural } from "@/lib/structure";
import { cn } from "@/lib/utils";
import { NodeExplorer } from "@/components/result/node-explorer";
import { documentExplorerAnalysis } from "@/lib/document";

function Shell({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={cn("rounded-xl bg-foreground/3 p-1.5 ring-1 ring-foreground/6 dark:bg-white/3", className)}>
      <div className="h-full rounded-xl bg-card p-4 shadow-[inset_0_1px_1px_rgba(255,255,255,0.6)] sm:p-5 dark:shadow-[inset_0_1px_1px_rgba(255,255,255,0.05)]">
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

/** Own vs. cumulative counts; the whole document has only one column. */
function CountsTable({ own, total }: { own?: DocumentCounts; total: DocumentCounts }) {
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-[10px] text-muted-foreground uppercase tracking-[0.18em]">
          <th className="pb-2 text-start font-medium">Count</th>
          {own && <th className="pb-2 text-end font-medium">Own</th>}
          <th className="pb-2 text-end font-medium">{own ? "With subsections" : "Total"}</th>
        </tr>
      </thead>
      <tbody className="divide-y divide-border/60">
        {COUNT_FIELDS.map(({ key, label }) => {
          const empty = !total[key];
          return (
            <tr className={cn(empty && "text-subtle-foreground")} key={key}>
              <td className="py-1.5">{label}</td>
              {own && <td className="py-1.5 text-end tabular-nums">{formatNumber(own[key])}</td>}
              <td className="py-1.5 text-end tabular-nums">{formatNumber(total[key])}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

export function DocumentResult({ result, onResetAction }: { result: DocumentAnalysis; onResetAction: () => void }) {
  const selected: string | null = null;
  const { document, structure, meta } = result;
  const totals = structure.totals;
  const index = useMemo(() => indexSections(structure.sections), [structure.sections]);
  const node = selected ? index.get(selected) : undefined;
  const json = useMemo(() => JSON.stringify(result, null, 2), [result]);
  const name = documentName(result);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(json);
      toastManager.add({
        title: "Copied JSON",
        description: `${formatBytes(json.length)} copied to the clipboard.`,
        type: "success",
      });
    } catch {
      toastManager.add({
        title: "Couldn't copy",
        description: "Clipboard access was blocked by the browser.",
        type: "error",
      });
    }
  };

  const download = () => {
    const url = URL.createObjectURL(new Blob([json], { type: "application/json" }));
    const a = Object.assign(window.document.createElement("a"), {
      href: url,
      download: `${
        name
          .replace(/[^\w-]+/g, "-")
          .replace(/^-+|-+$/g, "")
          .slice(0, 60) || "document"
      }.json`,
    });
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="book-page flex flex-col gap-6">
      <Reveal className="flex flex-col gap-6">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
          <div className="flex min-w-0 max-w-3xl flex-col gap-3">
            <div className="flex flex-wrap gap-1.5">
              <Badge variant="outline">{FORMAT_LABELS[document.format]}</Badge>
              {document.page_count != null && <Badge variant="secondary">{plural(document.page_count, "page")}</Badge>}
              <Badge variant="info">{meta.parser === "rust" ? "Rust parser" : "Python parser"}</Badge>
            </div>
            <h2 className="wrap-anywhere text-balance font-semibold text-3xl leading-[1.08] tracking-[-0.03em] sm:text-4xl">
              {name}
            </h2>
            <p className="text-muted-foreground text-sm">
              {formatBytes(document.size_bytes)} · {document.content_type} · analysed in{" "}
              {formatNumber(meta.processing_time_ms)} ms · {formatTimestamp(meta.analyzed_at)}
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-1">
            <Button aria-label="Copy JSON" className="rounded-lg" onClick={copy} size="icon" variant="ghost">
              <CopyIcon weight="light" />
            </Button>
            <Button aria-label="Download JSON" className="rounded-lg" onClick={download} size="icon" variant="ghost">
              <DownloadSimpleIcon weight="light" />
            </Button>
            <Button className="rounded-lg" onClick={onResetAction} variant="outline">
              <ArrowCounterClockwiseIcon weight="light" />
              Another document
            </Button>
          </div>
        </div>
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-6">
          <Stat label="Sections" value={formatNumber(totals.sections)} />
          <Stat label="Depth" value={formatNumber(totals.max_depth)} />
          <Stat label="Paragraphs" value={formatNumber(totals.paragraphs)} />
          <Stat label="Sentences" value={formatNumber(totals.sentences)} />
          <Stat label="Words" value={formatNumber(totals.words)} />
          <Stat label="Clauses" value={totals.clauses == null ? "Rescan needed" : formatNumber(totals.clauses)} />
        </div>
      </Reveal>

      <Reveal className="grid grid-cols-1 items-start gap-4 lg:grid-cols-12" delay={0.08}>
        <div className="lg:col-span-12">
          {structure.sections.some(section => section.paragraphs == null) && <p className="mb-3 text-sm text-muted-foreground">This older scan contains section totals only. Analyse the document again to explore sentences, clauses and words.</p>}
          <NodeExplorer analysis={documentExplorerAnalysis(result)} fullResult={result} />
        </div>
        <Shell className="lg:sticky lg:top-24 lg:col-span-5">
          <div className="mb-4 flex flex-col gap-1">
            <span className="text-[10px] text-muted-foreground uppercase tracking-[0.2em]">
              {node ? (node.kind === "preamble" ? "Preamble" : `Heading level ${node.level}`) : "Whole document"}
            </span>
            <h3 className="font-medium text-lg tracking-tight">{node ? sectionLabel(node) : name}</h3>
            {node && totals.words > 0 && (
              <p className="text-muted-foreground text-sm">
                {formatNumber((node.totals.words / totals.words) * 100, 1)}% of the document&apos;s words
                {node.children.length > 0 && ` · ${plural(node.children.length, "subsection")}`}
              </p>
            )}
          </div>
          <CountsTable own={node?.counts} total={node ? node.totals : totals} />
          <p className="mt-4 text-subtle-foreground text-xs">
            Counts and positions only: headings and text are never returned.
          </p>
        </Shell>
      </Reveal>
    </div>
  );
}
