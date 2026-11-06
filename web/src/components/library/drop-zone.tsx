"use client";

import { ArrowUpRightIcon, BookOpenTextIcon, FileArrowUpIcon } from "@phosphor-icons/react";
import { useQueryClient } from "@tanstack/react-query";
import { useRouter } from "next/navigation";
import { useEffect, useRef, useState } from "react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Progress,
  ProgressIndicator,
  ProgressLabel,
  ProgressTrack,
  ProgressValue,
} from "@/components/ui/progress";
import { LIBRARY_KEY } from "@/hooks/use-library";
import { stashAnalysis } from "@/lib/analysis-store";
import { type UploadProgress, uploadEpub, uploadDocument, DOCUMENT_ACCEPT, DOCUMENT_MAX_BYTES } from "@/lib/api/client";
import type { DocumentAnalysis } from "@/lib/api/types";
import { formatBytes } from "@/lib/format";
import { cn } from "@/lib/utils";

type State =
  | { phase: "idle" }
  | { phase: "uploading"; name: string; progress: UploadProgress; abort: () => void }
  | { phase: "error"; name: string; message: string };

const isEpub = (file: File) =>
  file.name.toLowerCase().endsWith(".epub") || file.type === "application/epub+zip";

/** One file picker dispatches each supported format to its parser. */
export function DropZone({ onDocument, onBusyChange }: { onDocument: (result: DocumentAnalysis) => void; onBusyChange: (busy: boolean) => void }) {
  const router = useRouter();
  const queryClient = useQueryClient();
  const input = useRef<HTMLInputElement>(null);
  const [dragging, setDragging] = useState(false);
  const [state, setState] = useState<State>({ phase: "idle" });
  const busy = state.phase === "uploading";
  useEffect(() => { onBusyChange(busy); }, [busy, onBusyChange]);

  const start = (file: File | undefined) => {
    if (!file || busy) return;
    if (!/\.(epub|html?|xhtml|md|markdown|pdf|txt|text)$/i.test(file.name)) {
      setState({ phase: "error", name: file.name, message: "Choose EPUB, PDF, HTML, Markdown or plain text." });
      return;
    }
    if (file.size > DOCUMENT_MAX_BYTES) {
      setState({ phase: "error", name: file.name, message: "That file exceeds the 20 MB limit. Choose a smaller file." });
      return;
    }
    const progress = (progress: UploadProgress) => setState((current) => current.phase === "uploading" ? { ...current, progress } : current);
    const { promise, abort } = isEpub(file) ? uploadEpub(file, progress) : uploadDocument(file, progress);
    setState({ phase: "uploading", name: file.name, progress: { loaded: 0, total: file.size }, abort });
    promise
      .then((analysis) => {
        queryClient.invalidateQueries({ queryKey: LIBRARY_KEY });
        if ("document" in analysis) {
          setState({ phase: "idle" });
          if (analysis.record_id) {
            queryClient.setQueryData(["saved", analysis.record_id], analysis);
            router.push(`/books/${analysis.record_id}`);
          } else onDocument(analysis);
          return;
        }
        if (analysis.record_id) {
          queryClient.setQueryData(["saved", analysis.record_id], analysis);
          router.push(`/books/${analysis.record_id}`);
        } else {
          stashAnalysis(analysis);
          router.push("/books/local");
        }
      })
      .catch((error: Error) => setState(error.message === "Upload cancelled." ? { phase: "idle" } : { phase: "error", name: file.name, message: error.message }));
  };

  const uploading = state.phase === "uploading" ? state : null;
  const sent = uploading && uploading.progress.phase === "processing";

  return (
    <div className="rounded-xl bg-foreground/[0.03] p-1.5 ring-1 ring-foreground/6 dark:bg-white/[0.03]">
      <div
        className={cn(
          "relative flex flex-col items-center gap-5 rounded-xl border border-dashed border-foreground/12 bg-card px-6 py-10 text-center shadow-[inset_0_1px_1px_rgba(255,255,255,0.6)] transition-[background-color,border-color,scale] duration-500 ease-spring dark:shadow-[inset_0_1px_1px_rgba(255,255,255,0.05)] sm:py-12",
          dragging && "scale-[0.99] border-primary/60 bg-brand-subtle",
        )}
        onDragLeave={() => setDragging(false)}
        onDragOver={(event) => {
          event.preventDefault();
          if (!busy) setDragging(true);
        }}
        onDrop={(event) => {
          event.preventDefault();
          setDragging(false);
          if (event.dataTransfer.files.length !== 1) {
            if (!busy) setState({ phase: "error", name: "these files", message: "Choose one file at a time." });
            return;
          }
          start(event.dataTransfer.files[0]);
        }}
      >
        <span className="flex size-14 items-center justify-center rounded-2xl bg-secondary ring-1 ring-foreground/6">
          {uploading ? (
            <BookOpenTextIcon className="size-7 text-primary" weight="light" />
          ) : (
            <FileArrowUpIcon className="size-7 text-muted-foreground" weight="light" />
          )}
        </span>

        {uploading ? (
          <div className="flex w-full max-w-sm flex-col gap-3" aria-live="polite">
            <Progress
              max={uploading.progress.total || 1}
              value={sent ? null : uploading.progress.loaded}
            >
              <div className="flex items-center justify-between gap-2">
                <ProgressLabel className="truncate">
                  {sent ? "Parsing structure…" : `Uploading ${uploading.name}`}
                </ProgressLabel>
                {!sent && <ProgressValue />}
              </div>
              <ProgressTrack className="h-1.5 rounded-lg">
                <ProgressIndicator className="rounded-lg" />
              </ProgressTrack>
            </Progress>
            <p className="text-subtle-foreground text-xs">
              {sent
                ? "Detecting the schema and counting sentences. Large books take a little while."
                : `${formatBytes(uploading.progress.loaded)} of ${formatBytes(uploading.progress.total)}`}
            </p>
            {uploading && (
              <Button className="self-center" onClick={uploading.abort} size="sm" variant="ghost">
                Cancel
              </Button>
            )}
          </div>
        ) : (
          <>
            <div className="flex flex-col gap-1.5">
              <p className="font-medium text-lg tracking-tight">Drop a book or document</p>
              <p className="text-muted-foreground text-sm">
                EPUB, PDF, HTML, Markdown or plain text. One file, up to 20 MB.
              </p>
            </div>
            <Button
              className="group h-11 rounded-lg ps-5 pe-1.5 active:scale-[0.98]"
              onClick={() => input.current?.click()}
              variant="outline"
            >
              Browse file
              <span className="flex size-8 items-center justify-center rounded-lg transition-transform duration-500 ease-spring group-hover:translate-x-0.5 group-hover:-translate-y-px group-hover:scale-105">
                <ArrowUpRightIcon weight="light" />
              </span>
            </Button>
          </>
        )}

        <input
          accept={`.epub,application/epub+zip,${DOCUMENT_ACCEPT}`}
          className="sr-only"
          onChange={(event) => {
            start(event.target.files?.[0]);
            event.target.value = "";
          }}
          ref={input}
          tabIndex={-1}
          type="file"
        />
      </div>
      {state.phase === "error" && (
        <Alert className="mt-1.5 rounded-xl" variant="error">
          <AlertTitle>Couldn&apos;t analyse {state.name}</AlertTitle>
          <AlertDescription>{state.message}</AlertDescription>
        </Alert>
      )}
    </div>
  );
}
