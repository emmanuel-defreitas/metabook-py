"use client";

import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { CaretRightIcon, TreeStructureIcon, TextAlignLeftIcon } from "@phosphor-icons/react";
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from "@/components/ui/collapsible";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Sidebar, SidebarProvider, SidebarHeader, SidebarContent, SidebarMenu, SidebarMenuItem, SidebarMenuButton, SidebarMenuSub } from "@/components/ui/sidebar";
import { useState } from "react";
import type { StructureDetail } from "@/lib/api/types";
import { childNodes, leafName, rootNodes, type TreeNode } from "@/lib/structure";
import { cn } from "@/lib/utils";

function WordBlock({ index, seed, tokens, onSelect }: { index: number; seed: string; tokens?: number; onSelect?: () => void }) {
  // Stable pseudo-random widths simulate words without implying original text length.
  const hash = Array.from(seed).reduce((value, char) => Math.imul(value ^ char.charCodeAt(0), 16777619), 2166136261) >>> 0;
  const width = 18 + hash % 49;
  return <span className="text-node word-node" data-kind="word">
    <span className="node-label" aria-hidden="true">Word {index}</span>
    <button type="button" onClick={onSelect} aria-label={`Word ${index}${tokens == null ? ", token count unavailable" : `, ${tokens} tokens in isolation`}`}
      title={`Word ${index}${tokens == null ? "" : ` · ${tokens} tokens (in isolation)`}`} className="word-silhouette" style={{ width }}>
      {tokens != null && <span className="token-badge">{tokens}</span>}
    </button>
  </span>;
}

function TextBlock({ node, leaf, onSelect }: { node: TreeNode; leaf: string; onSelect: (id: string) => void }) {
  const [limit, setLimit] = useState(80);
  const children = childNodes(node, leaf);
  const count = typeof node.raw.word_count === "number" ? node.raw.word_count : 0;
  if (node.kind === "word") return <WordBlock seed={node.id} index={Number(node.raw.index)} tokens={typeof node.raw.token_count === "number" ? node.raw.token_count : undefined} onSelect={() => onSelect(node.id)} />;
  return <span className="text-node" data-kind={node.kind}>
    <button type="button" onClick={() => onSelect(node.id)} title={node.meta} className="node-label">{node.label}</button>
    <span className="node-children">
      {children.length ? children.slice(0, limit).map(child => <TextBlock node={child} leaf={leaf} onSelect={onSelect} key={child.id} />) :
        Array.from({ length: Math.min(count, limit) }, (_, index) => <WordBlock key={index} seed={`${node.id}.${index}`} index={index + 1} onSelect={() => onSelect(node.id)} />)}
      {(children.length || count) > limit && <button type="button" className="mx-2 rounded text-xs text-primary focus-visible:ring-2 focus-visible:ring-ring" onClick={() => setLimit(limit + 80)}>Show more</button>}
      {!children.length && count === 0 && <span className="text-xs text-muted-foreground">No child detail in this scan</span>}
    </span>
  </span>;
}

function OutlineBranch({ node, leaf, selectedId, onSelect }: { node: TreeNode; leaf: string; selectedId: string | null; onSelect: (id: string) => void }) {
  const [open, setOpen] = useState(false);
  const [limit, setLimit] = useState(30);
  const children = open ? childNodes(node, leaf) : [];
  return <SidebarMenuItem>
    <Collapsible open={open} onOpenChange={setOpen}>
      <div className="flex items-center gap-0.5">
        {node.childCount > 0 ? <CollapsibleTrigger aria-label={`${open ? "Collapse" : "Expand"} ${node.label}`} className="flex size-7 shrink-0 items-center justify-center rounded-lg text-muted-foreground outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring"><CaretRightIcon weight="light" className={cn("size-3.5 transition-transform duration-150 motion-reduce:transition-none", open && "rotate-90")} /></CollapsibleTrigger> : <span className="w-7 shrink-0" />}
        <SidebarMenuButton type="button" isActive={node.id === selectedId} onClick={() => onSelect(node.id)} title={`${node.label} · ${node.meta}`} className="min-w-0 text-xs"><span className="truncate">{node.label}</span></SidebarMenuButton>
      </div>
      <CollapsiblePanel className="h-auto transition-none">
        <SidebarMenuSub className="mx-3 ps-2 pe-0">
          {children.slice(0, limit).map(child => <OutlineBranch key={child.id} node={child} leaf={leaf} selectedId={selectedId} onSelect={onSelect} />)}
          {children.length > limit && <SidebarMenuItem><SidebarMenuButton type="button" onClick={() => setLimit(limit + 30)}>Show 30 more</SidebarMenuButton></SidebarMenuItem>}
        </SidebarMenuSub>
      </CollapsiblePanel>
    </Collapsible>
  </SidebarMenuItem>;
}

/** Abstract word silhouettes preserve positions/counts without displaying prose. */
export function StructureTree({ structure, selectedId, onSelect }: {
  structure: StructureDetail; selectedId: string | null; onSelect: (id: string) => void;
}) {
  const roots = rootNodes(structure);
  const active = roots.find(node => node.id === selectedId?.split(".")[0]) ?? roots[0];
  const [limit, setLimit] = useState(12);
  const reduce = useReducedMotion();
  if (!active) return <p className="p-4 text-sm text-muted-foreground">This result has no structural nodes.</p>;
  const children = childNodes(active, leafName(structure));
  const selectedIndex = selectedId ? children.findIndex(node => selectedId === node.id || selectedId.startsWith(`${node.id}.`)) : -1;
  const shown = Math.max(limit, selectedIndex + 1);
  return <SidebarProvider className="book-explorer min-h-0! flex-col overflow-hidden rounded-[2rem] bg-foreground/[0.025] p-1.5 ring-1 ring-foreground/6 md:flex-row">
    <Sidebar collapsible="none" className="w-full! rounded-[1.625rem] bg-card md:w-64! lg:w-72!">
      <SidebarHeader className="flex-row items-center justify-between px-5 py-5"><span className="flex items-center gap-2 text-sm font-medium"><TreeStructureIcon weight="light" size={18} /> Outline</span><span className="font-mono text-xs text-muted-foreground">{roots.length}</span></SidebarHeader>
      <div className="flex h-52 min-h-0 flex-col md:h-[44rem]">
        <SidebarContent className="px-2 pb-4">
          <nav aria-label="Book outline"><SidebarMenu>{roots.map(node => <OutlineBranch key={node.id} node={node} leaf={leafName(structure)} selectedId={selectedId ?? active.id} onSelect={id => {setLimit(12);onSelect(id);}} />)}</SidebarMenu></nav>
        </SidebarContent>
      </div>
    </Sidebar>
    <section aria-label={`${active.label} children`} className="min-w-0 flex-1 rounded-[1.625rem] bg-card p-4 sm:p-6 md:ms-1.5 lg:p-8">
      <header className="mb-5 flex flex-wrap items-start justify-between gap-4">
        <div><p className="mb-2 flex items-center gap-2 text-xs text-muted-foreground"><TextAlignLeftIcon weight="light" size={16} /> Structure canvas</p><h2 className="book-content-font text-2xl font-medium">{active.label}</h2></div>
        <ul aria-label="Node type legend" className="flex flex-wrap gap-x-4 gap-y-2 text-xs">{['paragraph','sentence','clause','word'].map(kind => <li key={kind} data-kind={kind} className="node-legend"><span className="legend-swatch" />{kind}</li>)}</ul>
      </header>
      <p className="mb-5 text-xs text-muted-foreground">Hover a block to reveal its boundaries. Badges show each word’s token count.</p>
      <ScrollArea className="h-[36rem] md:h-[39rem]" overscrollContain>
        <AnimatePresence mode="wait" initial={false}>
          <motion.div key={active.id} initial={{ opacity: 0, y: reduce ? 0 : 8 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: reduce ? 0 : -4 }} transition={{ duration: reduce ? 0 : 0.18, ease: "easeOut" }} className="book-content-font px-2 pt-12 pb-4">
            {children.slice(0, shown).map(node => <TextBlock key={node.id} node={node} leaf={leafName(structure)} onSelect={onSelect} />)}
            {!children.length && <TextBlock node={active} leaf={leafName(structure)} onSelect={onSelect} />}
            {children.length > shown && <button className="rounded px-3 py-2 text-sm text-primary outline-none focus-visible:ring-2 focus-visible:ring-ring" onClick={() => setLimit(shown + 12)} type="button">Show next {Math.min(12, children.length - shown)} blocks</button>}
          </motion.div>
        </AnimatePresence>
      </ScrollArea>
    </section>
  </SidebarProvider>;
}
