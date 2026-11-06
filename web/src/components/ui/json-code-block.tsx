"use client";

import { useState, useTransition } from "react";

function JsonNode({ name, value, initialOpen = false }: { name: string; value: unknown; initialOpen?: boolean }) {
  const [open, setOpen] = useState(initialOpen);
  const [limit, setLimit] = useState(50);
  const [pending, startTransition] = useTransition();
  if (value === null || typeof value !== "object") {
    return <div className="break-all"><span className="text-primary">{JSON.stringify(name)}</span>: <span className={typeof value === "string" ? "text-success-foreground" : "text-info-foreground"}>{JSON.stringify(value)}</span></div>;
  }
  const array = Array.isArray(value);
  const keys = Object.keys(value);
  return (
    <div>
      <button aria-expanded={open} className="rounded py-1 text-left hover:bg-foreground/5 focus-visible:outline-2 focus-visible:outline-ring" onClick={() => startTransition(() => setOpen(!open))} type="button">
        <span aria-hidden="true">{open ? "▾" : "▸"} </span><span className="text-primary">{JSON.stringify(name)}</span>: {array ? "[" : "{"}<span className="text-muted-foreground"> {keys.length} {array ? "items" : "keys"} </span>{array ? "]" : "}"}
      </button>
      {pending && <div role="status" className="my-2 h-8 animate-pulse rounded bg-muted">Loading node…</div>}
      {open && <div className="ms-3 border-s border-border ps-3">
        {keys.slice(0, limit).map(key => <JsonNode key={key} name={key} value={(value as Record<string, unknown>)[key]} />)}
        {keys.length > limit && <button className="rounded py-2 text-primary" onClick={() => startTransition(() => setLimit(limit + 50))} type="button">Show next {Math.min(50, keys.length - limit)} items ({keys.length - limit} remaining)</button>}
      </div>}
    </div>
  );
}

/** Collapsed branches mount on demand, with bounded sibling rendering. */
export function JsonCodeBlock({ value }: { value: unknown }) {
  return <div className="overflow-x-auto rounded-xl bg-secondary/60 p-4 font-mono text-xs leading-relaxed" role="region" aria-label="JSON code"><JsonNode name="root" value={value} initialOpen /></div>;
}
