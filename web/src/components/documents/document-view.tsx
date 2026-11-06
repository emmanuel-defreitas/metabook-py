"use client";

import { ArrowRightIcon, GlobeSimpleIcon } from "@phosphor-icons/react";
import { useRouter } from "next/navigation";
import { useQueryClient } from "@tanstack/react-query";
import { LIBRARY_KEY } from "@/hooks/use-library";
import { useState } from "react";
import { Reveal } from "@/components/motion/reveal";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { DropZone } from "@/components/library/drop-zone";
import { Spinner } from "@/components/ui/spinner";
import {
  analyseDocumentUrl,
} from "@/lib/api/client";
import type { DocumentAnalysis } from "@/lib/api/types";
import { normalizeUrl } from "@/lib/document";
import { DocumentResult } from "./document-result";

type State =
  | { phase: "idle" }
  | { phase: "fetching"; url: string; abort: () => void }
  | { phase: "error"; source: string; message: string }
  | { phase: "done"; result: DocumentAnalysis };

const FORMATS = ["EPUB", "HTML", "Markdown", "PDF", "Plain text"];

/** Upload a file or fetch a public URL, then explore its section outline. */
export function DocumentView({ embedded = false }: { embedded?: boolean }) {
  const router = useRouter();
  const queryClient = useQueryClient();
  const [url, setUrl] = useState("");
  const [state, setState] = useState<State>({ phase: "idle" });
  const [uploadBusy, setUploadBusy] = useState(false);
  const busy = state.phase === "fetching" || uploadBusy;

  const fail = (source: string) => (error: Error) => {
    if (error.message === "Upload cancelled." || error.name === "AbortError") {
      setState({ phase: "idle" });
      return;
    }
    setState({ phase: "error", source, message: error.message });
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
      .then((result) => {
        queryClient.invalidateQueries({ queryKey: LIBRARY_KEY });
        if (result.record_id) {
          queryClient.setQueryData(["saved", result.record_id], result);
          router.push(`/books/${result.record_id}`);
        } else setState({ phase: "done", result });
      })
      .catch(fail(target));
  };

  const reset = () => {
    setState({ phase: "idle" });
    setUrl("");
  };

  return (
    <div className={embedded ? "flex min-w-0 flex-col gap-8" : "mx-auto flex w-full max-w-[1800px] flex-col gap-16 px-4 pt-16 pb-32 sm:px-8 sm:pt-24"}>
      {!embedded && <Reveal className="flex max-w-3xl flex-col gap-6">
        <span className="w-max rounded-lg bg-secondary px-3 py-1 font-medium text-[10px] text-muted-foreground uppercase tracking-[0.2em] ring-1 ring-foreground/6">
          Books & documents
        </span>
        <h1 className="text-balance font-semibold text-5xl leading-[1.02] tracking-[-0.035em] sm:text-6xl">
          Map a book or document.
        </h1>
        <p className="max-w-xl text-balance text-lg text-muted-foreground">
          Upload a file or point at a public page. Metabook nests its sections by heading level and counts paragraphs,
          lists, tables, links and words in each one.
        </p>
        <div className="flex flex-wrap gap-1.5">
          {FORMATS.map((format) => (
            <span
              className="rounded-lg px-2.5 py-0.5 text-muted-foreground text-xs ring-1 ring-foreground/8"
              key={format}
            >
              {format}
            </span>
          ))}
          <span className="px-1 py-0.5 text-subtle-foreground text-xs">up to 20 MB</span>
        </div>
      </Reveal>}

      {state.phase === "done" ? (
        <DocumentResult onResetAction={reset} result={state.result} />
      ) : (
        <Reveal className="grid grid-cols-1 gap-6 lg:grid-cols-12" delay={0.08}>
          <div className="lg:col-span-7" inert={state.phase === "fetching"}>
            <DropZone onBusyChange={setUploadBusy} onDocument={(result) => setState({ phase: "done", result })} />
          </div>

          <div className="rounded-xl bg-foreground/3 p-1.5 ring-1 ring-foreground/6 lg:col-span-5 dark:bg-white/3">
            <form
              className="flex h-full flex-col gap-5 rounded-xl bg-card p-6 shadow-[inset_0_1px_1px_rgba(255,255,255,0.6)] dark:shadow-[inset_0_1px_1px_rgba(255,255,255,0.05)]"
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
                  Public pages only. The API downloads it once, within 10 seconds. Parsed results are saved to your browser’s library.
                </p>
              </div>
              <InputGroup className="mt-auto rounded-lg">
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
                className="group h-11 w-full rounded-lg ps-5 pe-1.5 active:scale-[0.98]"
                disabled={busy || !url.trim()}
                type="submit"
              >
                <span className="flex-1 text-start">{state.phase === "fetching" ? "Fetching…" : "Analyse URL"}</span>
                <span className="flex size-8 items-center justify-center rounded-lg transition-transform duration-500 ease-spring group-hover:translate-x-0.5 group-hover:scale-105">
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
            <Alert className="rounded-xl lg:col-span-12" variant="error">
              <AlertTitle className="wrap-anywhere">Couldn&apos;t analyse {state.source}</AlertTitle>
              <AlertDescription>{state.message}</AlertDescription>
            </Alert>
          )}
        </Reveal>
      )}
    </div>
  );
}
