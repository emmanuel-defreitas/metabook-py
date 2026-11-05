"use client";

import { useState } from "react";
import type { Analysis } from "@/lib/api/types";
import { StructureTree } from "./structure-tree";
import { StructureMap } from "./structure-map";
import { JsonPane } from "./json-pane";

export function NodeExplorer({ analysis, fullResult }: { analysis: Analysis; fullResult?: unknown }) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [focusId, setFocusId] = useState<string | null>(null);
  return (
    <div className="grid min-w-0 gap-3 xl:grid-cols-[minmax(0,1fr)_19rem]">
      <StructureTree structure={analysis.structure} selectedId={selectedId} onSelect={id => {
        setSelectedId(id);
        setFocusId(id.includes(".") ? id.slice(0, id.lastIndexOf(".")) : id);
      }} />
      <aside aria-label="Node details" className="flex min-w-0 flex-col gap-3 xl:max-h-[max(40rem,calc(100svh-8rem))] xl:overflow-y-auto">
        <div className="rounded-xl bg-card p-3 ring-1 ring-border">
          <StructureMap analysis={analysis} focusId={focusId} selectedId={selectedId}
            onFocus={id => { setFocusId(id); if (id) setSelectedId(id); }} onSelect={setSelectedId} />
        </div>
        <div className="min-h-0 flex-1 rounded-xl bg-card p-3 ring-1 ring-border">
          <JsonPane analysis={analysis} selectedId={selectedId} fullResult={fullResult} />
        </div>
      </aside>
    </div>
  );
}
