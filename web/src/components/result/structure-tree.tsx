"use client";

import { CaretRightIcon } from "@phosphor-icons/react";
import { useMemo, useState } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { StructureDetail } from "@/lib/api/types";
import { ancestorIds, childNodes, leafName, rootNodes, type TreeNode } from "@/lib/structure";
import { cn } from "@/lib/utils";

const CHUNK = 100;

function Branch({
  nodes,
  depth,
  leaf,
  expanded,
  selectedId,
  onToggle,
  onSelect,
}: {
  nodes: TreeNode[];
  depth: number;
  leaf: string;
  expanded: Set<string>;
  selectedId: string | null;
  onToggle: (id: string) => void;
  onSelect: (id: string) => void;
}) {
  // Render long sibling lists in chunks; always include the selected node.
  const selectedIx = nodes.findIndex((n) => selectedId === n.id || selectedId?.startsWith(`${n.id}.`));
  const [limit, setLimit] = useState(Math.max(CHUNK, selectedIx + 1));
  const shown = nodes.slice(0, Math.max(limit, selectedIx + 1));
  return (
    <ul className="flex flex-col" role={depth === 0 ? "tree" : "group"}>
      {shown.map((node) => {
        const open = expanded.has(node.id);
        const selected = node.id === selectedId;
        return (
          <li aria-expanded={node.childCount ? open : undefined} aria-selected={selected} key={node.id} role="treeitem">
            <div
              className={cn(
                "group flex items-center gap-1 rounded-lg pe-2 text-sm transition-colors duration-200",
                selected ? "bg-primary/10 text-foreground" : "hover:bg-accent",
              )}
              style={{ paddingInlineStart: depth * 14 + 4 }}
            >
              <button
                aria-label={open ? `Collapse ${node.label}` : `Expand ${node.label}`}
                className={cn(
                  "flex size-6 shrink-0 items-center justify-center rounded-md text-muted-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring",
                  !node.childCount && "invisible",
                )}
                onClick={() => onToggle(node.id)}
                tabIndex={node.childCount ? 0 : -1}
                type="button"
              >
                <CaretRightIcon
                  className={cn("size-3.5 transition-transform duration-300 ease-spring", open && "rotate-90")}
                  weight="bold"
                />
              </button>
              <button
                className="flex min-w-0 flex-1 items-baseline gap-2 rounded-md py-1.5 text-start outline-none focus-visible:ring-2 focus-visible:ring-ring"
                onClick={() => onSelect(node.id)}
                type="button"
              >
                <span className={cn("max-w-[65%] shrink-0 truncate", selected && "font-medium")}>{node.label}</span>
                <span className="min-w-0 truncate text-subtle-foreground text-xs">{node.meta}</span>
              </button>
            </div>
            {open && node.childCount > 0 && (
              <Branch
                depth={depth + 1}
                expanded={expanded}
                leaf={leaf}
                nodes={childNodes(node, leaf)}
                onSelect={onSelect}
                onToggle={onToggle}
                selectedId={selectedId}
              />
            )}
          </li>
        );
      })}
      {nodes.length > shown.length && (
        <li>
          <button
            className="ms-8 rounded-md px-2 py-1.5 text-primary text-xs outline-none hover:underline focus-visible:ring-2 focus-visible:ring-ring"
            onClick={() => setLimit(shown.length + CHUNK)}
            style={{ marginInlineStart: depth * 14 + 32 }}
            type="button"
          >
            Show {Math.min(CHUNK, nodes.length - shown.length).toLocaleString()} more of{" "}
            {(nodes.length - shown.length).toLocaleString()}
          </button>
        </li>
      )}
    </ul>
  );
}

/** Lazily materialised structure tree; selecting a node also focuses the map. */
export function StructureTree({
  structure,
  selectedId,
  onSelect,
}: {
  structure: StructureDetail;
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  const leaf = leafName(structure);
  const roots = useMemo(() => rootNodes(structure), [structure]);
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set());

  // Reveal a selection made elsewhere (e.g. the map) by expanding its ancestors.
  const [revealed, setRevealed] = useState(selectedId);
  if (selectedId !== revealed) {
    setRevealed(selectedId);
    const ancestors = selectedId ? ancestorIds(selectedId) : [];
    if (!ancestors.every((id) => expanded.has(id))) {
      setExpanded(new Set([...expanded, ...ancestors]));
    }
  }

  const toggle = (id: string) =>
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  return (
    <ScrollArea className="h-[28rem]">
      <div className="p-1.5">
        {roots.length ? (
          <Branch
            depth={0}
            expanded={expanded}
            leaf={leaf}
            nodes={roots}
            onSelect={onSelect}
            onToggle={toggle}
            selectedId={selectedId}
          />
        ) : (
          <p className="p-4 text-subtle-foreground text-sm">This result has no structural nodes.</p>
        )}
      </div>
    </ScrollArea>
  );
}
