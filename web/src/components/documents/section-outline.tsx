"use client";

import { CaretRightIcon } from "@phosphor-icons/react";
import { useState } from "react";
import type { SectionNode } from "@/lib/api/types";
import { defaultExpanded, sectionLabel } from "@/lib/document";
import { formatNumber } from "@/lib/format";
import { plural } from "@/lib/structure";
import { cn } from "@/lib/utils";

/**
 * Section tree nested by heading level. Each row shows its share of the
 * document's words; selecting one shows its counts beside the tree.
 */
export function SectionOutline({
  sections,
  totalWords,
  selected,
  onSelectAction,
}: {
  sections: SectionNode[];
  totalWords: number;
  selected: string | null;
  onSelectAction: (path: string | null) => void;
}) {
  const [expanded, setExpanded] = useState(() => defaultExpanded(sections));

  const toggle = (path: string) =>
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });

  if (!sections.length) {
    return <p className="py-10 text-center text-sm text-subtle-foreground">No sections were found in this document.</p>;
  }

  const rows = (nodes: SectionNode[], depth: number): React.ReactNode =>
    nodes.map((node) => {
      const open = expanded.has(node.path);
      const share = totalWords ? node.totals.words / totalWords : 0;
      const isSelected = selected === node.path;
      return (
        <li
          aria-expanded={node.children.length ? open : undefined}
          key={node.path}
          role="treeitem"
          aria-selected={isSelected}
        >
          <div
            className={cn(
              "group flex items-center gap-1 rounded-xl pe-2 transition-colors duration-300 ease-spring",
              isSelected ? "bg-brand-subtle" : "hover:bg-accent/60",
            )}
            style={{ paddingInlineStart: `${depth * 1.25}rem` }}
          >
            {node.children.length ? (
              <button
                aria-label={open ? `Collapse ${sectionLabel(node)}` : `Expand ${sectionLabel(node)}`}
                className="flex size-7 shrink-0 items-center justify-center rounded-lg text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
                onClick={() => toggle(node.path)}
                type="button"
              >
                <CaretRightIcon
                  className={cn("size-3.5 transition-transform duration-300 ease-spring", open && "rotate-90")}
                  weight="light"
                />
              </button>
            ) : (
              <span className="size-7 shrink-0" />
            )}
            <button
              className="flex min-w-0 flex-1 items-center gap-3 rounded-lg py-2 text-start outline-none focus-visible:ring-2 focus-visible:ring-ring"
              onClick={() => onSelectAction(isSelected ? null : node.path)}
              type="button"
            >
              <span
                className={cn(
                  "flex h-5 min-w-7 shrink-0 items-center justify-center rounded-md px-1 font-mono text-[10px] ring-1",
                  node.kind === "preamble"
                    ? "text-subtle-foreground ring-foreground/8"
                    : "text-muted-foreground ring-foreground/10",
                )}
              >
                {node.kind === "preamble" ? "—" : `H${node.level}`}
              </span>
              <span className={cn("min-w-0 flex-1 truncate text-sm", isSelected && "font-medium")}>
                {sectionLabel(node)}
                {node.children.length > 0 && (
                  <span className="ms-2 text-subtle-foreground text-xs">
                    {plural(node.children.length, "subsection")}
                  </span>
                )}
              </span>
              <span className="hidden w-24 shrink-0 sm:block" aria-hidden>
                <span className="block h-1 overflow-hidden rounded-lg bg-foreground/6">
                  <span
                    className="block h-full rounded-lg bg-primary/70"
                    style={{ width: `${Math.max(share * 100, share > 0 ? 2 : 0)}%` }}
                  />
                </span>
              </span>
              <span className="w-20 shrink-0 text-end text-muted-foreground text-xs tabular-nums">
                {formatNumber(node.totals.words)} words
              </span>
            </button>
          </div>
          {open && node.children.length > 0 && <ul role="group">{rows(node.children, depth + 1)}</ul>}
        </li>
      );
    });

  return (
    <ul aria-label="Document sections" className="flex flex-col gap-0.5" role="tree">
      {rows(sections, 0)}
    </ul>
  );
}
