"use client";

import { BracketsCurlyIcon, CopyIcon, DownloadSimpleIcon } from "@phosphor-icons/react";
import { useMemo } from "react";
import { JsonCodeBlock } from "@/components/ui/json-code-block";
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

export function JsonPane({ analysis, selectedId, fullResult }: { analysis: Analysis; selectedId: string | null; fullResult?: unknown }) {
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
  const full = useMemo(() => JSON.stringify(fullResult ?? analysis, null, 2), [analysis, fullResult]);

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
        <p className="truncate font-medium text-sm">{node ? `${node.label}` : analysis.structure.schema === "document" ? "Document summary" : "Book summary"}</p>
        <div className="flex gap-1">
          <Dialog>
            <DialogTrigger render={<Button className="rounded-lg" size="sm" variant="outline" />}>
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
                <JsonCodeBlock value={fullResult ?? analysis} />
              </DialogPanel>
            </DialogPopup>
          </Dialog>
          <Button aria-label="Copy JSON" className="rounded-lg" onClick={copy} size="icon-sm" variant="ghost">
            <CopyIcon weight="light" />
          </Button>
          <Button aria-label="Download JSON" className="rounded-lg" onClick={download} size="icon-sm" variant="ghost">
            <DownloadSimpleIcon weight="light" />
          </Button>
        </div>
      </div>
      <ScrollArea className="h-64 rounded-xl bg-secondary/60">
        <div>
          <JsonCodeBlock value={JSON.parse(nodeJson)} />
        </div>
      </ScrollArea>
    </div>
  );
}
