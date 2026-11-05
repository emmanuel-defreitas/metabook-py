"use client";

import { Tabs } from "@base-ui/react/tabs";
import { motion, useReducedMotion } from "motion/react";
import { BookCover } from "./book-cover";
import { MatterPanel } from "./matter-panel";
import { ArrowLeftIcon } from "@phosphor-icons/react";
import Link from "next/link";
import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { analyseGutenberg } from "@/lib/api/client";
import { Button } from "@/components/ui/button";
import { SchemaScore } from "@/components/library/score";
import { Reveal } from "@/components/motion/reveal";
import { Badge } from "@/components/ui/badge";
import type { Analysis, UploadRecord } from "@/lib/api/types";
import { authorNames, schemaLabel } from "@/lib/format";
import { NodeExplorer } from "./node-explorer";
import { MetadataSections } from "./metadata";

export function ResultView({ analysis: initialAnalysis, record }: { analysis: Analysis; record?: UploadRecord }) {
  const [updated, setUpdated] = useState<Analysis | null>(null);
  const analysis = updated ?? initialAnalysis;
  const [refreshing, setRefreshing] = useState(false);
  const [refreshError, setRefreshError] = useState<string | null>(null);
  const queryClient = useQueryClient();
  const refresh = async () => {
    if (!analysis.book.gutenberg_id || refreshing) return;
    setRefreshing(true);
    setRefreshError(null);
    try {
      const result = await analyseGutenberg(analysis.book.gutenberg_id, undefined, "word");
      setUpdated(result);
      if (result.record_id) queryClient.setQueryData(["saved", result.record_id], result);
      await queryClient.invalidateQueries({ queryKey: ["library"] });
    } catch (error) {
      setRefreshError(error instanceof Error ? error.message : "Could not refresh this scan.");
    } finally {
      setRefreshing(false);
    }
  };
  const [tab, setTab] = useState("metadata");
  const reduce = useReducedMotion();
  const { book, structure } = analysis;

  const hasFront = structure.matter?.some((region) => region.position === "front") ?? false;
  const activeTab = tab === "front" && !hasFront ? "metadata" : tab;

  return (
    <Tabs.Root
      value={activeTab}
      onValueChange={(value) => setTab(String(value))}
      className="book-page mx-auto flex w-full max-w-[1800px] flex-col gap-8 px-4 pt-10 pb-24 sm:px-8 lg:px-12 sm:pt-14"
    >
      <Reveal className="flex flex-col gap-6">
        <div className="flex flex-wrap items-center gap-5">
          <Link
            className="flex w-max items-center gap-2 rounded-lg px-1 text-muted-foreground text-sm outline-none transition-colors hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
            href="/#library"
          >
            <ArrowLeftIcon weight="light" />
            Library
          </Link>
          <Tabs.List aria-label="Book views" className="relative flex flex-wrap gap-1 rounded-lg bg-secondary/50 p-1">
            {[
              ["metadata", "Metadata"],
              ...(hasFront ? [["front", "Front matter"]] : []),
              ["nodes", "Nodes"],
              ["back", "Back matter"],
            ].map(([value, label]) => (
              <Tabs.Tab
                key={value}
                value={value}
                className="relative rounded-sm px-4 py-2 text-sm text-muted-foreground outline-none transition-colors hover:text-foreground focus-visible:ring-0 focus-visible:ring-ring data-active:text-foreground"
              >
                {activeTab === value && (
                  <motion.span
                    layoutId="book-tab"
                    className="absolute inset-0 rounded-lg bg-card shadow-sm ring-1 ring-foreground/10"
                    transition={{ duration: reduce ? 0 : 0.22 }}
                  />
                )}
                <span className="relative">{label}</span>
              </Tabs.Tab>
            ))}
          </Tabs.List>
        </div>
        <div className="flex flex-wrap items-center gap-5 sm:gap-8">
          <BookCover id={book.gutenberg_id} title={book.title} />
          <div className="flex min-w-0 flex-1 flex-col gap-3">
            <div className="flex flex-wrap gap-1.5">
              <Badge variant="outline">{schemaLabel(structure.schema)}</Badge>
              <Badge
                variant={
                  structure.schema_confidence === "high"
                    ? "success"
                    : structure.schema_confidence === "low"
                      ? "warning"
                      : "info"
                }
              >
                {structure.schema_confidence} confidence
              </Badge>
              {book.gutenberg_id ? (
                <Badge variant="secondary">Gutenberg #{book.gutenberg_id}</Badge>
              ) : (
                <Badge variant="secondary">Uploaded EPUB</Badge>
              )}
            </div>
            <h1 className="font-[var(--font-book-content),sans-serif] text-balance text-3xl leading-[1.1] tracking-tight sm:text-5xl">
              {book.title}
            </h1>
            <p className="text-lg text-muted-foreground">{authorNames(book.authors)}</p>
          </div>
          {structure.schema_score > 0 && (
            <div className="w-full max-w-60">
              <SchemaScore score={structure.schema_score} />
            </div>
          )}
        </div>
      </Reveal>

      <div className="flex flex-wrap items-center gap-3 text-xs text-muted-foreground" aria-live="polite">
        {book.gutenberg_id ? (
          <Button size="sm" variant="outline" loading={refreshing} onClick={refresh}>
            Refresh word detail
          </Button>
        ) : null}
        <span>
          {refreshing ? "Scanning words, tokens and publication matter…" : "Token badges count each word in isolation."}
        </span>
        {refreshError && (
          <p role="alert" className="text-destructive-foreground">
            {refreshError}
          </p>
        )}
      </div>
      <Tabs.Panel value="metadata">
        <motion.div
          initial={{ opacity: 0, y: reduce ? 0 : 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: reduce ? 0 : 0.24 }}
        >
          <MetadataSections analysis={analysis} record={record} />
        </motion.div>
      </Tabs.Panel>
      {hasFront && (
        <Tabs.Panel value="front">
          <MatterPanel analysis={analysis} position="front" />
        </Tabs.Panel>
      )}
      <Tabs.Panel value="back">
        <MatterPanel analysis={analysis} position="back" />
      </Tabs.Panel>
      <Tabs.Panel value="nodes" className="space-y-6">
        <motion.div
          initial={{ opacity: 0, y: reduce ? 0 : 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: reduce ? 0 : 0.24 }}
          className="space-y-6"
        >
          <NodeExplorer analysis={analysis} />
        </motion.div>
      </Tabs.Panel>
    </Tabs.Root>
  );
}
