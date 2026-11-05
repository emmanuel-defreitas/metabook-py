"use client";

import { ArrowBendUpLeftIcon, CaretLeftIcon, CaretRightIcon } from "@phosphor-icons/react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import type { Analysis } from "@/lib/api/types";
import { childNodes, findNode, leafName, rootNodes, type TreeNode } from "@/lib/structure";
import { cn } from "@/lib/utils";

const PAGE = 6;
const SIZE = 560;
const CENTER = SIZE / 2;
const RADIUS = 200;
const EASE = [0.32, 0.72, 0, 1] as const;

function weight(node: TreeNode): number {
  const raw = node.raw;
  for (const key of ["total_words", "word_count"]) {
    if (typeof raw[key] === "number") return raw[key] as number;
  }
  return 1;
}

function short(label: string, max = 18) {
  return label.length > max ? `${label.slice(0, max - 1)}…` : label;
}

/**
 * Radial node map: the focused node in the middle, up to 6 children around it.
 * Click a child to select it; double-click (or "Open") to drill in.
 */
export function StructureMap({
  analysis,
  focusId,
  selectedId,
  onFocus,
  onSelect,
}: {
  analysis: Analysis;
  focusId: string | null;
  selectedId: string | null;
  onFocus: (id: string | null) => void;
  onSelect: (id: string) => void;
}) {
  const reduce = useReducedMotion();
  const structure = analysis.structure;
  const focus = focusId ? findNode(structure, focusId) : null;
  const children = useMemo(
    () => (focus ? childNodes(focus, leafName(structure)) : rootNodes(structure)),
    [focus, structure],
  );
  const [pageState, setPageState] = useState({ focusId, page: 0 });
  // Reset paging when the focus changes; jump to the page holding the selection.
  let page = pageState.focusId === focusId ? pageState.page : 0;
  if (pageState.focusId !== focusId) {
    const selectedIx = children.findIndex((c) => c.id === selectedId);
    page = selectedIx > 0 ? Math.floor(selectedIx / PAGE) : 0;
  }
  const pages = Math.max(1, Math.ceil(children.length / PAGE));
  const visible = children.slice(page * PAGE, page * PAGE + PAGE);
  const max = Math.max(...visible.map(weight), 1);
  const parentId = focusId?.includes(".") ? focusId.slice(0, focusId.lastIndexOf(".")) : null;

  const centerLabel = focus ? focus.label : analysis.book.title;
  const centerMeta = focus
    ? focus.meta
    : `${children.length.toLocaleString()} top-level ${children.length === 1 ? "node" : "nodes"}`;

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex min-w-0 items-center gap-2">
          <Button
            aria-label="Up one level"
            className="rounded-lg"
            disabled={!focusId}
            onClick={() => onFocus(parentId)}
            size="icon-sm"
            variant="outline"
          >
            <ArrowBendUpLeftIcon weight="light" />
          </Button>
          <p className="truncate text-muted-foreground text-sm">
            {focus ? (
              <>
                <span className="text-foreground">{focus.label}</span> · {focus.meta}
              </>
            ) : (
              analysis.structure.schema === "document" ? "Whole document" : "Whole book"
            )}
          </p>
        </div>
        {pages > 1 && (
          <div className="flex items-center gap-1 text-muted-foreground text-xs tabular-nums">
            <Button
              aria-label="Previous nodes"
              className="rounded-lg"
              disabled={page === 0}
              onClick={() => setPageState({ focusId, page: page - 1 })}
              size="icon-sm"
              variant="ghost"
            >
              <CaretLeftIcon weight="light" />
            </Button>
            {page * PAGE + 1}–{Math.min(children.length, (page + 1) * PAGE)} of {children.length.toLocaleString()}
            <Button
              aria-label="Next nodes"
              className="rounded-lg"
              disabled={page >= pages - 1}
              onClick={() => setPageState({ focusId, page: page + 1 })}
              size="icon-sm"
              variant="ghost"
            >
              <CaretRightIcon weight="light" />
            </Button>
          </div>
        )}
      </div>

      <div className="relative overflow-hidden rounded-xl bg-[radial-gradient(circle_at_center,color-mix(in_oklab,var(--primary)_7%,transparent),transparent_65%)]">
        <svg
          aria-label="Structure map"
          className="mx-auto block aspect-square w-full max-w-[560px]"
          role="group"
          viewBox={`0 0 ${SIZE} ${SIZE}`}
        >
          <AnimatePresence initial={false} mode="popLayout">
            <motion.g
              animate={{ opacity: 1, scale: 1 }}
              exit={reduce ? { opacity: 0 } : { opacity: 0, scale: 1.08 }}
              initial={reduce ? { opacity: 0 } : { opacity: 0, scale: 0.92 }}
              key={`${focusId ?? "root"}:${page}`}
              style={{ transformOrigin: `${CENTER}px ${CENTER}px` }}
              transition={{ duration: 0.6, ease: EASE }}
            >
              {visible.map((node, ix) => {
                const angle = (ix / Math.max(visible.length, 1)) * Math.PI * 2 - Math.PI / 2;
                const x = CENTER + Math.cos(angle) * RADIUS;
                const y = CENTER + Math.sin(angle) * RADIUS;
                return (
                  <line
                    className={cn(
                      "stroke-border transition-colors duration-300",
                      node.id === selectedId && "stroke-primary",
                    )}
                    key={`edge-${node.id}`}
                    strokeWidth={node.id === selectedId ? 1.5 : 1}
                    x1={CENTER}
                    x2={x}
                    y1={CENTER}
                    y2={y}
                  />
                );
              })}

              <g>
                <circle className="fill-card stroke-foreground/10" cx={CENTER} cy={CENTER} r={64} strokeWidth={1} />
                <circle className="fill-none stroke-primary/25" cx={CENTER} cy={CENTER} r={72} strokeDasharray="2 6" />
                <text className="fill-foreground font-semibold text-[18px]" textAnchor="middle" x={CENTER} y={CENTER - 2}>
                  {short(centerLabel, 14)}
                </text>
                <text className="fill-muted-foreground text-[16px]" textAnchor="middle" x={CENTER} y={CENTER + 16}>
                  {short(centerMeta.split(" · ")[0] ?? "", 18)}
                </text>
              </g>

              {visible.map((node, ix) => {
                const angle = (ix / Math.max(visible.length, 1)) * Math.PI * 2 - Math.PI / 2;
                const x = CENTER + Math.cos(angle) * RADIUS;
                const y = CENTER + Math.sin(angle) * RADIUS;
                const r = 14 + Math.sqrt(weight(node) / max) * 16;
                const selected = node.id === selectedId;
                const drillable = node.childCount > 0;
                const below = Math.sin(angle) > 0.2;
                return (
                  <g
                    aria-label={`${node.label}, ${node.meta}${drillable ? ". Press Enter to open." : ""}`}
                    aria-pressed={selected}
                    className="cursor-pointer outline-none [&:focus-visible>circle:first-child]:stroke-ring"
                    key={node.id}
                    onClick={() => onSelect(node.id)}
                    onDoubleClick={() => drillable && onFocus(node.id)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" && drillable) onFocus(node.id);
                      if (event.key === " ") {
                        event.preventDefault();
                        onSelect(node.id);
                      }
                    }}
                    role="button"
                    tabIndex={0}
                  >
                    <title>{`${node.label} · ${node.meta}`}</title>
                    <circle
                      className={cn(
                        "transition-[fill,stroke] duration-300",
                        selected ? "fill-primary stroke-primary" : "fill-card stroke-foreground/15 hover:stroke-primary/60",
                      )}
                      cx={x}
                      cy={y}
                      r={r}
                      strokeWidth={selected ? 2 : 1}
                    />
                    {drillable && (
                      <circle
                        className={cn(selected ? "fill-primary-foreground" : "fill-mark")}
                        cx={x}
                        cy={y}
                        r={3}
                      />
                    )}
                    <text
                      className={cn("text-[18px]", selected ? "fill-foreground font-medium" : "fill-muted-foreground")}
                      textAnchor="middle"
                      x={x}
                      y={below ? y + r + 14 : y - r - 8}
                    >
                      {short(node.label, 14)}
                    </text>
                  </g>
                );
              })}
            </motion.g>
          </AnimatePresence>
        </svg>
        {children.length === 0 && (
          <p className="absolute inset-x-0 bottom-6 text-center text-subtle-foreground text-xs">
            This node has no deeper structure at the scanned detail level.
          </p>
        )}
      </div>
      <p className="text-subtle-foreground text-xs">
        Select a node to inspect it. Nodes with a green dot have children: double-click or press Enter to open them.
      </p>
    </div>
  );
}
