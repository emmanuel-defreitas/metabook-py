"use client";

import { BracketsCurlyIcon, CopyIcon, DownloadSimpleIcon } from "@phosphor-icons/react";
import { useMemo } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogDescription,
  DialogHeader,
  DialogPanel,
  DialogPopup,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { toastManager } from "@/components/ui/toast";
import type { Analysis } from "@/lib/api/types";
import { formatBytes } from "@/lib/format";
import { findNode, nodeFields } from "@/lib/structure";

/** Rendering more than this in the dialog stalls the page; download has it all. */
const DIALOG_LIMIT = 400_000;

const TOKEN = /("(?:\\.|[^"\\])*")(\s*:)?|\b(true|false|null)\b|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)/g;

function Highlighted({ json }: { json: string }) {
  const parts = useMemo(() => {
    const out: React.ReactNode[] = [];
    let last = 0;
    for (const match of json.matchAll(TOKEN)) {
      const index = match.index ?? 0;
      if (index > last) out.push(json.slice(last, index));
      const [whole, str, colon, literal, number] = match;
      const className = str
        ? colon
          ? "text-primary"
          : "text-success-foreground"
        : literal
          ? "text-warning-foreground"
          : number
            ? "text-info-foreground"
            : "";
      out.push(
        <span className={className} key={index}>
          {colon ? str : whole}
        </span>,
      );
      if (colon) out.push(colon);
      last = index + whole.length;
    }
    out.push(json.slice(last));
    return out;
  }, [json]);
  return <>{parts}</>;
}

export function JsonPane({ analysis, selectedId }: { analysis: Analysis; selectedId: string | null }) {
  const node = selectedId ? findNode(analysis.structure, selectedId) : null;
  const nodeJson = useMemo(
    () =>
      JSON.stringify(
        node ? { id: node.id, ...nodeFields(node.raw) } : { book: analysis.book, summary: analysis.structure.summary },
        null,
        2,
      ),
    [node, analysis],
  );
  const full = useMemo(() => JSON.stringify(analysis, null, 2), [analysis]);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(full);
      toastManager.add({ title: "Copied JSON", description: `${formatBytes(full.length)} copied to the clipboard.`, type: "success" });
    } catch {
      toastManager.add({ title: "Couldn't copy", description: "Clipboard access was blocked by the browser.", type: "error" });
    }
  };

  const download = () => {
    const url = URL.createObjectURL(new Blob([full], { type: "application/json" }));
    const a = document.createElement("a");
    a.href = url;
    a.download = `${(analysis.book.title || "metabook").replace(/[^\w-]+/g, "-").slice(0, 60)}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="flex h-full flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="truncate font-medium text-sm">{node ? `${node.label}` : "Book summary"}</p>
        <div className="flex gap-1">
          <Dialog>
            <DialogTrigger render={<Button className="rounded-full" size="sm" variant="outline" />}>
              <BracketsCurlyIcon weight="light" />
              Full JSON
            </DialogTrigger>
            <DialogPopup className="max-w-4xl">
              <DialogHeader>
                <DialogTitle>Full JSON</DialogTitle>
                <DialogDescription>
                  The complete API result ({formatBytes(full.length)}). It holds positions and counts only.
                </DialogDescription>
              </DialogHeader>
              <DialogPanel>
                <pre className="overflow-x-auto font-mono text-xs leading-relaxed">
                  {full.length > DIALOG_LIMIT ? (
                    <>
                      {full.slice(0, DIALOG_LIMIT)}
                      {"\n\n… truncated for display. Download the file for the complete result."}
                    </>
                  ) : (
                    <Highlighted json={full} />
                  )}
                </pre>
              </DialogPanel>
            </DialogPopup>
          </Dialog>
          <Button aria-label="Copy JSON" className="rounded-full" onClick={copy} size="icon-sm" variant="ghost">
            <CopyIcon weight="light" />
          </Button>
          <Button aria-label="Download JSON" className="rounded-full" onClick={download} size="icon-sm" variant="ghost">
            <DownloadSimpleIcon weight="light" />
          </Button>
        </div>
      </div>
      <ScrollArea className="h-[25.5rem] rounded-xl bg-secondary/60">
        <pre className="p-4 font-mono text-xs leading-relaxed">
          <Highlighted json={nodeJson} />
        </pre>
      </ScrollArea>
    </div>
  );
}
