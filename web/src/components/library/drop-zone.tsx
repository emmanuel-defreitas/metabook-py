"use client";

import { ArrowUpRightIcon, BookOpenTextIcon, FileArrowUpIcon } from "@phosphor-icons/react";
import { useQueryClient } from "@tanstack/react-query";
import { useRouter } from "next/navigation";
import { useRef, useState } from "react";
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
import { type UploadProgress, uploadEpub } from "@/lib/api/client";
import { formatBytes } from "@/lib/format";
import { cn } from "@/lib/utils";

type State =
  | { phase: "idle" }
  | { phase: "uploading"; name: string; progress: UploadProgress; abort: () => void }
  | { phase: "error"; name: string; message: string };

const isEpub = (file: File) =>
  file.name.toLowerCase().endsWith(".epub") || file.type === "application/epub+zip";

/** Browse or drop one EPUB; analysis starts immediately. */
export function DropZone() {
  const router = useRouter();
  const queryClient = useQueryClient();
  const input = useRef<HTMLInputElement>(null);
  const [dragging, setDragging] = useState(false);
  const [state, setState] = useState<State>({ phase: "idle" });
  const busy = state.phase === "uploading";

  const start = (file: File | undefined) => {
    if (!file || busy) return;
    if (!isEpub(file)) {
      setState({ phase: "error", name: file.name, message: "Choose an .epub file." });
      return;
    }
    const { promise, abort } = uploadEpub(file, (progress) =>
      setState((current) => (current.phase === "uploading" ? { ...current, progress } : current)),
    );
    setState({ phase: "uploading", name: file.name, progress: { loaded: 0, total: file.size }, abort });
    promise
      .then((analysis) => {
        queryClient.invalidateQueries({ queryKey: LIBRARY_KEY });
        if (analysis.record_id) {
          queryClient.setQueryData(["saved", analysis.record_id], analysis);
          router.push(`/books/${analysis.record_id}`);
        } else {
          stashAnalysis(analysis);
          router.push("/books/local");
        }
      })
      .catch((error: Error) => setState({ phase: "error", name: file.name, message: error.message }));
  };

  const uploading = state.phase === "uploading" ? state : null;
  const sent = uploading && uploading.progress.loaded >= uploading.progress.total;

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
            {!sent && (
              <Button className="self-center" onClick={uploading.abort} size="sm" variant="ghost">
                Cancel
              </Button>
            )}
          </div>
        ) : (
          <>
            <div className="flex flex-col gap-1.5">
              <p className="font-medium text-lg tracking-tight">Drop an EPUB to map its structure</p>
              <p className="text-muted-foreground text-sm">
                Chapters, paragraphs and sentences, counted. The text itself never leaves the parser.
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
          accept=".epub,application/epub+zip"
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
