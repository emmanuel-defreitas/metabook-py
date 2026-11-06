"use client";

import { ArrowLeftIcon, ArrowRightIcon, ArrowUpRightIcon, MagnifyingGlassIcon, TreeStructureIcon } from "@phosphor-icons/react";
import { useQuery } from "@tanstack/react-query";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useMemo, useState } from "react";
import { SchemaScore } from "@/components/library/score";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { useLibrary } from "@/hooks/use-library";
import { catalogSearch, openAlexUrl } from "@/lib/api/client";
import type { CatalogResult } from "@/lib/api/types";
import { filtersFromParams, filtersToParams, SOURCE_OPTIONS, type SearchFilters } from "@/lib/filters";
import { CatalogSearch } from "./catalog-search";

export function SearchResultsSkeleton() {
  return (
    <div className="mx-auto flex w-full max-w-5xl flex-col gap-4 px-4 pt-12 sm:px-8">
      <Skeleton className="h-10 rounded-lg" />
      {Array.from({ length: 5 }, (_, i) => (
        <Skeleton className="h-24 rounded-xl" key={i} />
      ))}
    </div>
  );
}

function subtitle(result: CatalogResult) {
  const source = result.source === "gutendex" ? `Gutendex · #${result.gutenberg_id}` : `OpenAlex · ${result.id}`;
  return [source, result.kind, result.language, result.publication_date].filter(Boolean).join(" · ");
}

function ResultRow({ result, score }: { result: CatalogResult; score: number | null | undefined }) {
  const isGutenberg = result.source === "gutendex" && result.gutenberg_id;
  const external = result.source === "openalex" ? openAlexUrl(result.id) : null;
  const body = (
    <>
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <h3 className="truncate font-medium tracking-tight">{result.title}</h3>
        <p className="truncate text-muted-foreground text-sm">
          {result.authors.length ? result.authors.join(", ") : "Author not provided"}
        </p>
        <p className="truncate text-subtle-foreground text-xs">{subtitle(result)}</p>
      </div>
      <div className="flex shrink-0 items-center gap-4">
        {isGutenberg && (
          <div className="max-sm:hidden">
            <SchemaScore compact score={score} />
          </div>
        )}
        <span className="inline-flex h-8 items-center gap-1.5 rounded-lg bg-secondary ps-3 pe-1 font-medium text-sm transition-colors duration-300 ease-spring group-hover:bg-primary group-hover:text-primary-foreground">
          {isGutenberg ? "Schema" : "Open record"}
          <span className="flex size-6 items-center justify-center rounded-lg bg-foreground/5 transition-transform duration-500 ease-spring group-hover:translate-x-0.5 group-hover:-translate-y-px dark:bg-white/10">
            {isGutenberg ? <TreeStructureIcon weight="light" /> : <ArrowUpRightIcon weight="light" />}
          </span>
        </span>
      </div>
    </>
  );
  const className =
    "group flex items-center gap-4 rounded-xl bg-card px-5 py-4 outline-none ring-1 ring-foreground/6 transition-[box-shadow,scale] duration-500 ease-spring hover:shadow-[0_18px_40px_-28px_rgba(15,18,23,0.35)] focus-visible:ring-2 focus-visible:ring-ring active:scale-[0.995]";
  if (isGutenberg) {
    return (
      <Link className={className} href={`/gutenberg/${result.gutenberg_id}`}>
        {body}
      </Link>
    );
  }
  if (external) {
    return (
      <a className={className} href={external} rel="noreferrer noopener" target="_blank">
        {body}
      </a>
    );
  }
  return <div className={className}>{body}</div>;
}

export function SearchResults() {
  const params = useSearchParams();
  // The submitted snapshot lives in the URL; a new submission remounts the
  // editable filters so they start from it. Edits apply on the next Enter.
  return <Results key={params.toString()} params={params} />;
}

function Results({ params }: { params: URLSearchParams }) {
  const router = useRouter();
  const q = (params.get("q") ?? "").trim();
  const page = Math.max(1, Number(params.get("page")) || 1);
  const submitted = useMemo(() => filtersFromParams(params), [params]);
  const [filters, setFilters] = useState<SearchFilters>(submitted);

  const library = useLibrary();
  const search = useQuery({
    queryKey: ["catalog", q, page, submitted],
    queryFn: ({ signal }) => catalogSearch({ q, page, ...submitted }, signal),
    enabled: q.length > 0,
    placeholderData: (previous) => previous,
  });

  const goTo = (target: number) => {
    const next = filtersToParams(submitted);
    next.set("q", q);
    next.set("page", String(target));
    router.push(`/search?${next}`);
    window.scrollTo({ top: 0, behavior: "smooth" });
  };

  const sourceLabel = SOURCE_OPTIONS.find((o) => o.value === submitted.source)?.label ?? "Gutendex";
  const scoreFor = (id?: number | null) =>
    library.data?.find((record) => record.gutenberg_id === id)?.scan?.schema_score;

  return (
    <div className="mx-auto flex w-full max-w-5xl flex-col gap-10 px-4 pt-10 pb-32 sm:px-8 sm:pt-14">
      <CatalogSearch filters={filters} initialQuery={q} onFiltersChange={setFilters} />

      {!q ? (
        <Empty className="py-20">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <MagnifyingGlassIcon weight="light" />
            </EmptyMedia>
            <EmptyTitle>Search the catalog</EmptyTitle>
            <EmptyDescription>Enter catalog keywords, an ISBN, or a Gutenberg ID, then press Enter.</EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : search.isPending ? (
        <div className="flex flex-col gap-3">
          {Array.from({ length: 6 }, (_, i) => (
            <Skeleton className="h-22 rounded-xl" key={i} />
          ))}
        </div>
      ) : search.isError ? (
        <Alert variant="error">
          <AlertTitle>Search failed</AlertTitle>
          <AlertDescription>{search.error.message}</AlertDescription>
        </Alert>
      ) : (
        <section aria-busy={search.isFetching} className="flex flex-col gap-6">
          <div className="flex flex-col gap-2">
            <p className="text-muted-foreground text-sm">
              {search.data.count === 0
                ? `No catalog results in ${sourceLabel}. Try another query or change the filters.`
                : `${search.data.count.toLocaleString()} results · ${sourceLabel} · Page ${search.data.page}`}
            </p>
            <p className="text-subtle-foreground text-xs">
              Gutenberg: Schema analyses and saves the book. OpenAlex: Open record visits the catalog; it
              doesn&apos;t scan or save a schema.
            </p>
            {search.data.warnings.map((warning) => (
              <Badge className="w-max max-w-full whitespace-normal" key={warning} variant="warning">
                {warning}
              </Badge>
            ))}
          </div>
          <div className="flex flex-col gap-3">
            {search.data.results.map((result) => (
              <ResultRow key={`${result.source}-${result.id}`} result={result} score={scoreFor(result.gutenberg_id)} />
            ))}
          </div>
          {(search.data.previous_page || search.data.next_page) && (
            <nav aria-label="Result pages" className="flex items-center justify-between gap-2">
              <Button
                className="rounded-lg"
                disabled={!search.data.previous_page}
                onClick={() => search.data.previous_page && goTo(search.data.previous_page)}
                variant="outline"
              >
                <ArrowLeftIcon weight="light" />
                Previous
              </Button>
              <Button
                className="rounded-lg"
                disabled={!search.data.next_page}
                onClick={() => search.data.next_page && goTo(search.data.next_page)}
                variant="outline"
              >
                Next
                <ArrowRightIcon weight="light" />
              </Button>
            </nav>
          )}
        </section>
      )}
    </div>
  );
}
