"use client";

import { motion, useReducedMotion } from "motion/react";
import type { Analysis } from "@/lib/api/types";

export function MatterPanel({ analysis, position }: { analysis: Analysis; position: "front" | "back" }) {
  const reduce = useReducedMotion();
  const matter = analysis.structure.matter;
  const regions = matter?.filter(region => region.position === position) ?? [];
  const title = position === "front" ? "Front matter" : "Back matter";
  return <motion.section initial={{ opacity: 0, y: reduce ? 0 : 8 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: reduce ? 0 : 0.24 }} className="rounded-xl bg-card p-6 ring-1 ring-foreground/8">
    <h2 className="text-xl font-medium">{title}</h2>
    <p className="mt-2 text-sm text-muted-foreground">Publication material outside explicit book-body boundaries. Main content is never shown.</p>
    {regions.length ? <div className="mt-6 grid gap-4">{regions.map(region => <article key={`${region.start}-${region.end}`} className="rounded-lg bg-secondary/50 p-5">
      <h3 className="font-medium">{region.kind.replaceAll("_", " ")}</h3>
      {region.text ? <div className="mt-4 max-h-[32rem] overflow-auto whitespace-pre-wrap break-words rounded border border-border bg-background/50 p-4 text-sm leading-relaxed">{region.text}</div> : <p className="mt-4 text-sm text-muted-foreground">Text was not retained in this saved scan.</p>}
      <p className="mt-3 text-2xl tabular-nums">{region.word_count.toLocaleString()} <span className="text-sm text-muted-foreground">words</span></p>
      <p className="mt-3 text-sm text-muted-foreground">{region.evidence}</p>
      <p className="mt-1 text-xs text-muted-foreground">Source characters {region.start.toLocaleString()}–{region.end.toLocaleString()} · before cleanup</p>
    </article>)}</div> : <div className="mt-6 rounded-lg border border-dashed border-border p-6">
      <h3 className="font-medium">{matter == null ? "Not available in this saved scan" : `No confirmed ${position} matter detected`}</h3>
      <p className="mt-2 max-w-xl text-sm text-muted-foreground">{matter == null ? "This scan did not retain excluded publication material. A new scan is needed to detect it." : "No explicit Gutenberg boundary markers identified this material. Unmarked publisher pages and authored prefaces are not automatically classified."}</p>
    </div>}
  </motion.section>;
}
