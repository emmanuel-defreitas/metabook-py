"use client";

import { AnimatePresence, motion, useMotionValue, useReducedMotion } from "motion/react";
import { CaretRightIcon, TreeStructureIcon, TextAlignLeftIcon, SidebarSimpleIcon, InfoIcon } from "@phosphor-icons/react";
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Tooltip, TooltipTrigger, TooltipPopup } from "@/components/ui/tooltip";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Sidebar,
  SidebarProvider,
  SidebarHeader,
  SidebarContent,
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
  SidebarMenuSub,
} from "@/components/ui/sidebar";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import type { StructureDetail } from "@/lib/api/types";
import { childNodes, leafName, rootNodes, type TreeNode } from "@/lib/structure";
import { cn } from "@/lib/utils";

const ROOT_CHUNK = 100;

function WordBlock({
  index,
  seed,
  nodeId = seed,
  selectedId,
  tokens,
  onSelect,
}: {
  index: number;
  seed: string;
  nodeId?: string;
  selectedId: string | null;
  tokens?: number;
  onSelect?: () => void;
}) {
  // Stable pseudo-random widths simulate words without implying original text length.
  const hash =
    Array.from(seed).reduce((value, char) => Math.imul(value ^ char.charCodeAt(0), 16777619), 2166136261) >>> 0;
  const width = 18 + (hash % 49);
  return (
    <span
      className="canvas-node group/word relative inline-flex rounded-xl border border-transparent p-0.5 mx-0.5 my-[3px] align-middle transition-[border-color,background-color] duration-150 ease-[cubic-bezier(0.2,0,0,1)] hover:border-[color-mix(in_srgb,var(--node-color)_65%,transparent)] hover:bg-[color-mix(in_srgb,var(--node-color)_3%,transparent)] focus-within:border-[color-mix(in_srgb,var(--node-color)_65%,transparent)] focus-within:bg-[color-mix(in_srgb,var(--node-color)_3%,transparent)] motion-reduce:transition-none"
      data-kind="word"
      data-canvas-node={nodeId}
      data-active={selectedId === seed}
    >
      <button
        type="button"
        onClick={onSelect}
        aria-label={`Word ${index}${tokens == null ? ", token count unavailable" : `, ${tokens} tokens in isolation`}`}
        className="relative inline-block h-5 rounded-[3px] bg-[color-mix(in_srgb,var(--foreground)_18%,transparent)] outline-none focus-visible:outline-2 focus-visible:outline-[var(--node-color)] focus-visible:outline-offset-[3px]"
        style={{ width }}
      >
        {tokens != null && (
          <span data-kind="token" className="token-badge absolute -right-0.5 -top-[5px] grid place-items-center min-w-[13px] h-[13px] px-0.5 rounded-[2px] text-foreground text-[8px] leading-none bg-secondary shadow-[0_0_0_1px_color-mix(in_srgb,var(--node-color)_35%,transparent)]">
            {tokens}
          </span>
        )}
      </button>
    </span>
  );
}

const textNodeBase =
  "canvas-node group/text relative border border-transparent rounded-xs transition-[border-color,background-color] duration-150 ease-[cubic-bezier(0.2,0,0,1)] hover:border-[color-mix(in_srgb,var(--node-color)_65%,transparent)] hover:bg-[color-mix(in_srgb,var(--node-color)_3%,transparent)] focus-within:border-[color-mix(in_srgb,var(--node-color)_65%,transparent)] focus-within:bg-[color-mix(in_srgb,var(--node-color)_3%,transparent)] motion-reduce:transition-none";

const textNodeKindClasses: Record<string, string> = {
  paragraph: "block p-[3px] mb-2 leading-none border-none",
  chapter: "block p-[3px] mb-8 leading-[2.4]",
  section: "block p-[3px] mb-8 leading-[2.4]",
  sentence: "inline p-0.5 [box-decoration-break:clone]",
  clause: "inline [box-decoration-break:clone]",
};

function TextBlock({ node, leaf, onSelect, selectedId }: { node: TreeNode; leaf: string; onSelect: (id: string) => void; selectedId: string | null }) {
  const [limit, setLimit] = useState(80);
  const children = childNodes(node, leaf);
  const selectedChild = children.findIndex(child => selectedId === child.id || selectedId?.startsWith(`${child.id}.`));
  const shown = Math.max(limit, selectedChild + 1);
  const count = typeof node.raw.word_count === "number" ? node.raw.word_count : 0;
  if (node.kind === "word")
    return (
      <WordBlock
        seed={node.id}
        selectedId={selectedId}
        index={Number(node.raw.index)}
        tokens={typeof node.raw.token_count === "number" ? node.raw.token_count : undefined}
        onSelect={() => onSelect(node.id)}
      />
    );
  if (node.raw.role === "heading") return (
    <div className="my-2 p-[3px]" data-canvas-node={node.id} aria-label="Detected heading">
      <span className="inline-block h-6 w-56 max-w-[70%] rounded-[3px] bg-foreground/25" aria-hidden="true" />
    </div>
  );
  if (node.raw.role === "illustration") return (
    <div className="my-2 flex h-12 w-1/2 items-center justify-center gap-2 rounded border border-dashed border-border p-[3px] text-xs text-muted-foreground" data-canvas-node={node.id}>
      Illustration
    </div>
  );
  return (
    <span className={cn(textNodeBase, textNodeKindClasses[node.kind])} data-kind={node.kind} data-canvas-node={node.id} data-active={selectedId === node.id}>
      <span>
        {children.length
          ? children
              .slice(0, shown)
              .map((child) => <TextBlock node={child} leaf={leaf} selectedId={selectedId} onSelect={onSelect} key={child.id} />)
          : Array.from({ length: Math.min(count, limit) }, (_, index) => (
              <WordBlock
                key={index}
                seed={`${node.id}.placeholder${index}`}
                nodeId={node.id}
                selectedId={selectedId}
                index={index + 1}
                onSelect={() => onSelect(node.id)}
              />
            ))}
        {(children.length || count) > limit && (
          <button
            type="button"
            className="mx-1 rounded text-xs text-primary focus-visible:ring-0 focus-visible:ring-ring"
            onClick={() => setLimit(limit + 80)}
          >
            Show more
          </button>
        )}
        {!children.length && count === 0 && (
          <span className="text-xs text-muted-foreground">No child detail in this scan</span>
        )}
      </span>
    </span>
  );
}

function OutlineBranch({
  node,
  leaf,
  selectedId,
  onSelect,
}: {
  node: TreeNode;
  leaf: string;
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [limit, setLimit] = useState(30);
  const expanded = open || Boolean(selectedId?.startsWith(`${node.id}.`));
  const children = expanded ? childNodes(node, leaf) : [];
  const selectedChild = children.findIndex(child => selectedId === child.id || selectedId?.startsWith(`${child.id}.`));
  const shown = Math.max(limit, selectedChild + 1);
  return (
    <SidebarMenuItem>
      <Collapsible open={expanded} onOpenChange={value => {
        setOpen(value);
        if (!value && selectedId?.startsWith(`${node.id}.`)) onSelect(node.id);
      }}>
        <div className="flex items-center gap-0.5">
          {node.childCount > 0 ? (
            <CollapsibleTrigger
              aria-label={`${expanded ? "Collapse" : "Expand"} ${node.label}`}
              className="flex size-7 shrink-0 items-center justify-center rounded-lg text-muted-foreground outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring"
            >
              <CaretRightIcon
                weight="light"
                className={cn(
                  "size-3.5 transition-transform duration-150 motion-reduce:transition-none",
                  expanded && "rotate-90",
                )}
              />
            </CollapsibleTrigger>
          ) : (
            <span className="w-7 shrink-0" />
          )}
          <SidebarMenuButton
            type="button"
            isActive={node.id === selectedId}
            data-kind={node.kind}
            data-outline-node={node.id}
            onClick={() => onSelect(node.id)}
            title={`${node.label} · ${node.meta}`}
            className="outline-node min-w-0 text-xs"
          >
            <span className="truncate">{node.label}</span>
          </SidebarMenuButton>
        </div>
        <CollapsiblePanel className="motion-reduce:transition-none">
          <SidebarMenuSub className="mx-3 ps-2 pe-0">
            {children.slice(0, shown).map((child) => (
              <OutlineBranch key={child.id} node={child} leaf={leaf} selectedId={selectedId} onSelect={onSelect} />
            ))}
            {children.length > shown && (
              <SidebarMenuItem>
                <SidebarMenuButton type="button" onClick={() => setLimit(limit + 30)}>
                  Show 30 more
                </SidebarMenuButton>
              </SidebarMenuItem>
            )}
          </SidebarMenuSub>
        </CollapsiblePanel>
      </Collapsible>
    </SidebarMenuItem>
  );
}

/** Abstract word silhouettes preserve positions/counts without displaying prose. */
export function StructureTree({
  structure,
  selectedId,
  onSelect,
  inspector,
}: {
  inspector?: ReactNode;
  structure: StructureDetail;
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  const roots = rootNodes(structure);
  const active = roots.find((node) => node.id === selectedId?.split(".")[0]) ?? roots[0];

  const [rootLimit, setRootLimit] = useState(ROOT_CHUNK);
  const [outlineOpen, setOutlineOpen] = useState(true);
  const [tooltip, setTooltip] = useState<string[]>([]);
  const tooltipX = useMotionValue(0);
  const tooltipY = useMotionValue(0);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const showTooltip = (target: HTMLElement, x: number, y: number) => {
    const path: string[] = [];
    let node = target.closest<HTMLElement>(".canvas-node");
    while (node) {
      const kind = node.dataset.kind;
      if (kind && ["sentence", "clause", "word"].includes(kind)) path.unshift(kind);
      node = node.parentElement?.closest<HTMLElement>(".canvas-node") ?? null;
    }
    setTooltip(current => current.join() === path.join() ? current : path);
    const width = tooltipRef.current?.offsetWidth ?? 240;
    tooltipX.set(Math.max(8, Math.min(x - width / 2, window.innerWidth - width - 8)));
    tooltipY.set(y < 60 ? y + 24 : y - 48);
  };
  const reduce = useReducedMotion();
  const explorer = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLElement>(null);
  const [scrollRequest, setScrollRequest] = useState<{ id: string } | null>(null);
  useEffect(() => {
    const root = explorer.current;
    if (!root) return;
    const item = root.querySelector<HTMLElement>(`[data-outline-node="${selectedId}"]`);
    const viewport = item?.closest<HTMLElement>('[data-slot="scroll-area-viewport"]');
    if (item && viewport) {
      const itemRect = item.getBoundingClientRect();
      const viewRect = viewport.getBoundingClientRect();
      if (itemRect.top < viewRect.top || itemRect.bottom > viewRect.bottom) {
        viewport.scrollTop += itemRect.top - viewRect.top - viewport.clientHeight / 2;
      }
    }
  }, [selectedId]);
  useEffect(() => {
    const root = canvas.current;
    if (!root || !scrollRequest) return;
    const scroll = () => {
      const target = root.querySelector<HTMLElement>(`[data-canvas-node="${scrollRequest.id}"]`);
      const viewport = root.querySelector<HTMLElement>('[data-slot="scroll-area-viewport"]');
      if (!viewport) return false;
      if (!target && scrollRequest.id.includes(".")) return false;
      const top = target ? target.getBoundingClientRect().top - viewport.getBoundingClientRect().top + viewport.scrollTop - 40 : 0;
      viewport.scrollTo({ top, behavior: reduce ? "instant" : "smooth" });
      return true;
    };
    const observer = new MutationObserver(() => { if (scroll()) observer.disconnect(); });
    if (!scroll()) observer.observe(root, { childList: true, subtree: true });
    return () => observer.disconnect();
  }, [scrollRequest, reduce]);
  if (!active) return <p className="p-4 text-sm text-muted-foreground">This result has no structural nodes.</p>;
  const rootsShown = Math.max(rootLimit, roots.indexOf(active) + 1);
  const children = childNodes(active, leafName(structure));
  const activeIndex = roots.indexOf(active);
  const navigate = (id: string) => { onSelect(id); setScrollRequest({ id }); setTooltip([]); };
  return (
    <SidebarProvider ref={explorer} className="book-explorer min-h-0! flex-col overflow-hidden rounded-2xl bg-foreground/2.5 p-1.5 ring-1 ring-foreground/6 md:flex-row md:flex-wrap xl:flex-nowrap xl:h-[max(40rem,calc(100svh-8rem))]">
      <div className="outline-shell" data-open={outlineOpen} inert={!outlineOpen}>
      <Sidebar id="book-outline" collapsible="none" className="w-full! min-h-0 overflow-hidden rounded-xl bg-card md:h-full! md:w-64! lg:w-72!">
        <SidebarHeader className="flex-row items-center justify-between px-5 py-5">
          <span className="flex items-center gap-2 text-sm font-medium">
            <TreeStructureIcon weight="light" size={18} /> Outline
          </span>
          <span className="font-mono text-xs text-muted-foreground">{roots.length}</span>
        </SidebarHeader>
        <div className="flex h-52 min-h-0 flex-col md:h-0 md:flex-1">
          <SidebarContent className="px-2 pb-4">
            <nav aria-label={structure.schema === "document" ? "Document outline" : "Book outline"}>
              <SidebarMenu>
                {roots.slice(0, rootsShown).map((node) => (
                  <OutlineBranch
                    key={node.id}
                    node={node}
                    leaf={leafName(structure)}
                    selectedId={selectedId ?? active.id}
                    onSelect={(id) => {
                      onSelect(id);
                      setScrollRequest({ id });
                    }}
                  />
                ))}
                {roots.length > rootsShown && (
                  <SidebarMenuItem>
                    <SidebarMenuButton type="button" className="text-xs text-primary" onClick={() => setRootLimit(rootsShown + ROOT_CHUNK)}>
                      Show {Math.min(ROOT_CHUNK, roots.length - rootsShown).toLocaleString()} more of {roots.length.toLocaleString()}
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                )}
              </SidebarMenu>
            </nav>
          </SidebarContent>
        </div>
      </Sidebar>
      </div>
      <section
        ref={canvas}
        onPointerMove={event => {
          if (event.pointerType === "touch") return;
          showTooltip(event.target as HTMLElement, event.clientX, event.clientY);
          const node = (event.target as HTMLElement).closest<HTMLElement>("[data-canvas-node]");
          if (node?.dataset.canvasNode && node.dataset.canvasNode.split(".")[0] === active.id && node.dataset.canvasNode !== selectedId) onSelect(node.dataset.canvasNode);
        }}
        onPointerLeave={() => setTooltip([])}
        onScrollCapture={() => setTooltip([])}
        onKeyDown={event => { if (event.key === "Escape") setTooltip([]); }}
        onFocusCapture={event => {
          const target = event.target as HTMLElement;
          const rect = target.getBoundingClientRect();
          showTooltip(target, rect.left + rect.width / 2, rect.top);
        }}
        onBlurCapture={() => setTooltip([])}
        aria-label={`${active.label} children`}
        className="flex min-h-0 min-w-0 flex-1 flex-col rounded-xl bg-card p-4 sm:p-6 md:ms-1.5 lg:p-8"
      >
        <div className="sticky top-0 z-10 mb-4 shrink-0 border-b border-border bg-card pb-4">
        <header className="mb-3 flex flex-wrap items-start justify-between gap-4">
          <div>
            <p className="mb-2 flex items-center gap-2 text-xs text-muted-foreground">
              <button type="button" aria-label={outlineOpen ? "Collapse outline" : "Expand outline"} aria-expanded={outlineOpen} aria-controls="book-outline" onClick={() => setOutlineOpen(value => !value)} className="rounded p-1 hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring"><SidebarSimpleIcon size={18} /></button>
              <TextAlignLeftIcon weight="light" size={16} /> Structure canvas
            </p>
            <div className="flex items-center gap-2">
              <h2 className="font-[var(--font-book-content),sans-serif] text-2xl">{active.label}</h2>
              <Tooltip>
                <TooltipTrigger aria-label={structure.schema === "document" ? "Section statistics" : "Chapter statistics"} className="rounded p-1 text-muted-foreground hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"><InfoIcon size={16} /></TooltipTrigger>
                <TooltipPopup><dl className="grid grid-cols-2 gap-x-5 gap-y-1 p-2">
                  {[["Paragraphs", active.raw.paragraph_count], ["Sentences", active.raw.total_sentences], ["Clauses", active.raw.total_clauses], ["Words", active.raw.total_words], ["Tokens", active.raw.total_tokens]].filter(([, value]) => typeof value === "number").map(([label, value]) => <div key={String(label)} className="contents"><dt>{String(label)}</dt><dd className="text-right tabular-nums">{Number(value).toLocaleString()}</dd></div>)}
                </dl></TooltipPopup>
              </Tooltip>
            </div>
          </div>
          <ul aria-label="Node type legend" className="flex flex-wrap gap-x-3 gap-y-1 text-xs">
            {["paragraph", "sentence", "clause", "word", "token"].map((kind) => (
              <li key={kind} data-kind={kind} className="inline-flex items-center gap-1 text-muted-foreground capitalize">
                <span className="w-2.5 h-2.5 rounded-[3px] border border-(--node-color) bg-[color-mix(in_srgb,var(--node-color)_12%,transparent)]" />
                {kind}
              </li>
            ))}
          </ul>
        </header>
        <p className="text-xs text-muted-foreground">
          {structure.schema === "document" ? "Hover to inspect a node. Click the outline to scroll to it. Token counts are not included in document scans." : "Hover to inspect a node. Click the outline to scroll to it. Token badges appear on word hover."}
        </p>
        </div>
        <ScrollArea className="h-144 xl:h-0 xl:flex-1" overscrollContain>
          <AnimatePresence mode="wait" initial={false}>
            <motion.div
              key={active.id}
              initial={{ opacity: 0, y: reduce ? 0 : 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: reduce ? 0 : -4 }}
              transition={{ duration: reduce ? 0 : 0.18, ease: "easeOut" }}
              className="font-[var(--font-book-content),sans-serif] px-2"
            >
              {active.kind !== "paragraph" && active.raw.has_heading !== false && (
                <div className="mb-3 px-[3px]" aria-label={`Detected heading: ${active.label}`}>
                  <span className="inline-block h-6 w-56 max-w-[70%] rounded-[3px] bg-foreground/25" aria-hidden="true" />
                </div>
              )}
              {children.map((node) => (
                <TextBlock key={node.id} node={node} leaf={leafName(structure)} selectedId={selectedId} onSelect={onSelect} />
              ))}
              {!children.length && <TextBlock node={active} leaf={leafName(structure)} selectedId={selectedId} onSelect={onSelect} />}
              <nav aria-label="Chapter navigation" className="mt-6 flex items-center justify-between gap-3 border-t border-border pt-4">
                <button type="button" disabled={activeIndex === 0} onClick={() => navigate(roots[activeIndex - 1].id)} className="rounded px-3 py-2 text-sm hover:bg-accent disabled:opacity-40 focus-visible:ring-2 focus-visible:ring-ring">
                  Previous {active.kind === "chapter" ? "chapter" : "section"}
                </button>
                <span className="text-xs text-muted-foreground">{activeIndex + 1} / {roots.length}</span>
                <button type="button" disabled={activeIndex === roots.length - 1} onClick={() => navigate(roots[activeIndex + 1].id)} className="rounded px-3 py-2 text-sm hover:bg-accent disabled:opacity-40 focus-visible:ring-2 focus-visible:ring-ring">
                  Next {active.kind === "chapter" ? "chapter" : "section"}
                </button>
              </nav>
            </motion.div>
          </AnimatePresence>
        </ScrollArea>
      </section>
      {inspector && <div className="w-full min-w-0 xl:ms-1.5 xl:flex xl:w-72 xl:shrink-0">{inspector}</div>}
      {typeof document !== "undefined" && createPortal(
        <AnimatePresence>
          {tooltip.length > 0 && <motion.div
            ref={tooltipRef}
            role="tooltip"
            aria-label={tooltip.join(" > ")}
            className="book-page pointer-events-none fixed z-50 rounded border border-border bg-popover px-3 py-2 text-xs text-popover-foreground shadow-md"
            style={{ left: tooltipX, top: tooltipY }}
            initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
            transition={{ duration: reduce ? 0 : 0.12 }}
          >
            <span className="flex items-center gap-2">
              {tooltip.map((kind, index) => <span key={kind} className="inline-flex items-center gap-2">
                {index > 0 && <span className="text-muted-foreground">›</span>}
                <span data-kind={kind} className="text-(--node-color) capitalize">{kind}</span>
              </span>)}
            </span>
            <span className="absolute left-1/2 top-full h-2 w-px bg-border" />
          </motion.div>}
        </AnimatePresence>, document.body)}
    </SidebarProvider>
  );
}
