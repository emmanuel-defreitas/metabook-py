"use client";

import { ArrowRightIcon, FileArrowUpIcon, FileTextIcon, GlobeSimpleIcon } from "@phosphor-icons/react";
import { useRef, useState } from "react";
import { Reveal } from "@/components/motion/reveal";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import {
  Progress,
  ProgressIndicator,
  ProgressLabel,
  ProgressTrack,
  ProgressValue,
} from "@/components/ui/progress";
import { Spinner } from "@/components/ui/spinner";
import { analyseDocumentUrl, DOCUMENT_ACCEPT, type UploadProgress, uploadDocument } from "@/lib/api/client";
import type { DocumentAnalysis } from "@/lib/api/types";
import { normalizeUrl } from "@/lib/document";
import { formatBytes } from "@/lib/format";
import { cn } from "@/lib/utils";
import { DocumentResult } from "./document-result";

type State =
  | { phase: "idle" }
  | { phase: "uploading"; name: string; progress: UploadProgress; abort: () => void }
  | { phase: "fetching"; url: string; abort: () => void }
  | { phase: "error"; source: string; message: string }
  | { phase: "done"; result: DocumentAnalysis };

const FORMATS = ["HTML", "Markdown", "PDF", "Plain text"];

/** Upload a file or fetch a public URL, then explore its section outline. */
export function DocumentView() {
  const input = useRef<HTMLInputElement>(null);
  const [dragging, setDragging] = useState(false);
  const [url, setUrl] = useState("");
  const [state, setState] = useState<State>({ phase: "idle" });
  const busy = state.phase === "uploading" || state.phase === "fetching";

  const fail = (source: string) => (error: Error) => {
    if (error.message === "Upload cancelled." || error.name === "AbortError") {
      setState({ phase: "idle" });
      return;
    }
    setState({ phase: "error", source, message: error.message });
  };

  const upload = (file: File | undefined) => {
    if (!file || busy) return;
    const { promise, abort } = uploadDocument(file, (progress) =>
      setState((current) => (current.phase === "uploading" ? { ...current, progress } : current)),
    );
    setState({ phase: "uploading", name: file.name, progress: { loaded: 0, total: file.size }, abort });
    promise.then((result) => setState({ phase: "done", result })).catch(fail(file.name));
  };

  const fetchUrl = () => {
    if (busy) return;
    const target = normalizeUrl(url);
    if (!target) {
      setState({ phase: "error", source: url || "that URL", message: "Enter a public http(s) URL." });
      return;
    }
    const controller = new AbortController();
    setState({ phase: "fetching", url: target, abort: () => controller.abort() });
    analyseDocumentUrl(target, controller.signal)
      .then((result) => setState({ phase: "done", result }))
      .catch(fail(target));
  };

  const reset = () => {
    setState({ phase: "idle" });
    setUrl("");
  };

  const uploading = state.phase === "uploading" ? state : null;
  const sent = uploading && uploading.progress.loaded >= uploading.progress.total;

  return (
    <div className="mx-auto flex w-full max-w-6xl flex-col gap-16 px-4 pt-16 pb-32 sm:px-8 sm:pt-24">
      <Reveal className="flex max-w-3xl flex-col gap-6">
        <span className="w-max rounded-full bg-secondary px-3 py-1 font-medium text-[10px] text-muted-foreground uppercase tracking-[0.2em] ring-1 ring-foreground/6">
          Single documents
        </span>
        <h1 className="text-balance font-semibold text-5xl leading-[1.02] tracking-[-0.035em] sm:text-6xl">
          Outline any document.
        </h1>
        <p className="max-w-xl text-balance text-lg text-muted-foreground">
          Upload a file or point at a public page. Metabook nests its sections by heading level and
          counts paragraphs, lists, tables, links and words in each one.
        </p>
        <div className="flex flex-wrap gap-1.5">
          {FORMATS.map((format) => (
            <span className="rounded-full px-2.5 py-0.5 text-muted-foreground text-xs ring-1 ring-foreground/8" key={format}>
              {format}
            </span>
          ))}
          <span className="px-1 py-0.5 text-subtle-foreground text-xs">up to 20 MB</span>
        </div>
      </Reveal>

      {state.phase === "done" ? (
        <DocumentResult onReset={reset} result={state.result} />
      ) : (
        <Reveal className="grid grid-cols-1 gap-6 lg:grid-cols-12" delay={0.08}>
          <div className="rounded-[2rem] bg-foreground/[0.03] p-1.5 ring-1 ring-foreground/6 lg:col-span-7 dark:bg-white/[0.03]">
            <div
              className={cn(
                "relative flex h-full flex-col items-center justify-center gap-5 rounded-[calc(2rem-0.375rem)] border border-dashed border-foreground/12 bg-card px-6 py-10 text-center shadow-[inset_0_1px_1px_rgba(255,255,255,0.6)] transition-[background-color,border-color,scale] duration-500 ease-spring dark:shadow-[inset_0_1px_1px_rgba(255,255,255,0.05)] sm:py-12",
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
                upload(event.dataTransfer.files[0]);
              }}
            >
              <span className="flex size-14 items-center justify-center rounded-2xl bg-secondary ring-1 ring-foreground/6">
                {uploading ? (
                  <FileTextIcon className="size-7 text-primary" weight="light" />
                ) : (
                  <FileArrowUpIcon className="size-7 text-muted-foreground" weight="light" />
                )}
              </span>
              {uploading ? (
                <div aria-live="polite" className="flex w-full max-w-sm flex-col gap-3">
                  <Progress max={uploading.progress.total || 1} value={sent ? null : uploading.progress.loaded}>
                    <div className="flex items-center justify-between gap-2">
                      <ProgressLabel className="truncate">
                        {sent ? "Building the outline…" : `Uploading ${uploading.name}`}
                      </ProgressLabel>
                      {!sent && <ProgressValue />}
                    </div>
                    <ProgressTrack className="h-1.5 rounded-full">
                      <ProgressIndicator className="rounded-full" />
                    </ProgressTrack>
                  </Progress>
                  <p className="text-subtle-foreground text-xs">
                    {formatBytes(uploading.progress.loaded)} of {formatBytes(uploading.progress.total)}
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
                    <p className="font-medium text-lg tracking-tight">Drop a document</p>
                    <p className="text-muted-foreground text-sm">HTML, Markdown, PDF or plain text. Nothing is saved.</p>
                  </div>
                  <Button
                    className="group h-11 rounded-full ps-5 pe-1.5 active:scale-[0.98]"
                    disabled={busy}
                    onClick={() => input.current?.click()}
                    variant="outline"
                  >
                    Browse files
                    <span className="flex size-8 items-center justify-center rounded-full bg-foreground/5 transition-transform duration-500 ease-spring group-hover:translate-x-0.5 group-hover:-translate-y-px group-hover:scale-105 dark:bg-white/10">
                      <ArrowRightIcon weight="light" />
                    </span>
                  </Button>
                </>
              )}
              <input
                accept={DOCUMENT_ACCEPT}
                className="sr-only"
                onChange={(event) => {
                  upload(event.target.files?.[0]);
                  event.target.value = "";
                }}
                ref={input}
                tabIndex={-1}
                type="file"
              />
            </div>
          </div>

          <div className="rounded-[2rem] bg-foreground/[0.03] p-1.5 ring-1 ring-foreground/6 lg:col-span-5 dark:bg-white/[0.03]">
            <form
              className="flex h-full flex-col gap-5 rounded-[calc(2rem-0.375rem)] bg-card p-6 shadow-[inset_0_1px_1px_rgba(255,255,255,0.6)] dark:shadow-[inset_0_1px_1px_rgba(255,255,255,0.05)]"
              onSubmit={(event) => {
                event.preventDefault();
                fetchUrl();
              }}
            >
              <span className="flex size-10 items-center justify-center rounded-xl bg-secondary">
                <GlobeSimpleIcon className="size-5" weight="light" />
              </span>
              <div className="flex flex-col gap-1.5">
                <h2 className="font-medium text-lg tracking-tight">Or fetch a URL</h2>
                <p className="text-muted-foreground text-sm">
                  Public pages only. The API downloads it once, within 10 seconds, and keeps nothing.
                </p>
              </div>
              <InputGroup className="mt-auto rounded-full">
                <InputGroupAddon>
                  {state.phase === "fetching" ? <Spinner /> : <GlobeSimpleIcon weight="light" />}
                </InputGroupAddon>
                <InputGroupInput
                  aria-label="Document URL"
                  autoComplete="url"
                  disabled={busy}
                  enterKeyHint="go"
                  inputMode="url"
                  onChange={(event) => setUrl(event.target.value)}
                  placeholder="https://example.com/paper.pdf"
                  value={url}
                />
              </InputGroup>
              <Button
                className="group h-11 w-full rounded-full ps-5 pe-1.5 active:scale-[0.98]"
                disabled={busy || !url.trim()}
                type="submit"
              >
                <span className="flex-1 text-start">{state.phase === "fetching" ? "Fetching…" : "Analyse URL"}</span>
                <span className="flex size-8 items-center justify-center rounded-full bg-primary-foreground/15 transition-transform duration-500 ease-spring group-hover:translate-x-0.5 group-hover:scale-105">
                  <ArrowRightIcon weight="light" />
                </span>
              </Button>
              {state.phase === "fetching" && (
                <Button className="-mt-3 self-center" onClick={state.abort} size="sm" type="button" variant="ghost">
                  Cancel
                </Button>
              )}
            </form>
          </div>

          {state.phase === "error" && (
            <Alert className="rounded-[1.5rem] lg:col-span-12" variant="error">
              <AlertTitle className="wrap-anywhere">Couldn&apos;t analyse {state.source}</AlertTitle>
              <AlertDescription>{state.message}</AlertDescription>
            </Alert>
          )}
        </Reveal>
      )}
    </div>
  );
}
