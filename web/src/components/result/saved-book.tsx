"use client";

import { useQuery } from "@tanstack/react-query";
import { useRouter } from "next/navigation";
import { DocumentResult } from "@/components/documents/document-result";
import { useState } from "react";
import { useLibrary } from "@/hooks/use-library";
import { takeStashedAnalysis } from "@/lib/analysis-store";
import { openSaved } from "@/lib/api/client";
import { ResultError, ResultSkeleton } from "./result-states";
import { ResultView } from "./result-view";

/** Open a committed result without another scan. `local` = unsaved upload. */
export function SavedBook({ id }: { id: string }) {
  const router = useRouter();
  const [stashed] = useState(() => (id === "local" ? takeStashedAnalysis() : null));
  const library = useLibrary();
  const query = useQuery({
    queryKey: ["saved", id],
    queryFn: ({ signal }) => openSaved(id, signal),
    enabled: id !== "local",
    staleTime: Number.POSITIVE_INFINITY,
  });
  const record = library.data?.find((entry) => entry.id === id);

  if (id === "local") {
    return stashed ? (
      <ResultView analysis={stashed} />
    ) : (
      <ResultError message="This unsaved result is gone. Upload the EPUB again to re-analyse it." />
    );
  }
  if (query.isPending) return <ResultSkeleton message="Opening saved result…" />;
  if (query.isError) return <ResultError message={query.error.message} onRetry={() => query.refetch()} />;
  if ("document" in query.data) return <div className="mx-auto w-full max-w-[1800px] px-6 py-24"><DocumentResult result={query.data} onResetAction={() => router.push("/documents")} /></div>;
  return <ResultView analysis={query.data} record={record} />;
}
