"use client";

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useRouter } from "next/navigation";
import { useEffect } from "react";
import { LIBRARY_KEY } from "@/hooks/use-library";
import { analyseGutenberg } from "@/lib/api/client";
import { ResultError, ResultSkeleton } from "./result-states";
import { ResultView } from "./result-view";

/** Analyse (and save) one Gutenberg book, then move to its saved URL. */
export function GutenbergBook({ id }: { id: number }) {
  const router = useRouter();
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: ["gutenberg", id],
    queryFn: ({ signal }) => analyseGutenberg(id, signal),
    enabled: Number.isInteger(id) && id > 0,
    staleTime: Number.POSITIVE_INFINITY,
    retry: false,
  });

  const recordId = query.data?.record_id;
  useEffect(() => {
    if (!query.data) return;
    queryClient.invalidateQueries({ queryKey: LIBRARY_KEY });
    if (recordId) {
      queryClient.setQueryData(["saved", recordId], query.data);
      router.replace(`/books/${encodeURIComponent(recordId)}`);
    }
  }, [query.data, recordId, queryClient, router]);

  if (!Number.isInteger(id) || id <= 0) return <ResultError message="That isn't a valid Gutenberg ID." />;
  if (query.isPending) return <ResultSkeleton message="Analysing the book structure… this can take a minute for long books." />;
  if (query.isError) return <ResultError message={query.error.message} onRetry={() => query.refetch()} />;
  return <ResultView analysis={query.data} />;
}
