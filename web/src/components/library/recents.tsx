"use client";

import { ArrowClockwiseIcon, BooksIcon } from "@phosphor-icons/react";
import { useMemo } from "react";
import { Reveal } from "@/components/motion/reveal";
import { Alert, AlertAction, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { useLibrary } from "@/hooks/use-library";
import { matchesRecord, recordedActivity, type SearchFilters } from "@/lib/filters";
import { BookCard } from "./book-card";

/** Saved books, newest activity first, filtered live by the shared filters. */
export function Recents({ filters, query }: { filters: SearchFilters; query: string }) {
  const library = useLibrary();
  // The fetch time is the reference clock: pure, and consistent across cards.
  const now = library.dataUpdatedAt;
  const books = useMemo(() => {
    return (library.data ?? [])
      .filter((record) => matchesRecord(record, filters, query, now))
      .sort((a, b) => (recordedActivity(b, now) ?? -1) - (recordedActivity(a, now) ?? -1));
  }, [library.data, filters, query, now]);

  return (
    <section aria-labelledby="library-heading" className="flex flex-col gap-8" id="library">
      <div className="flex items-end justify-between gap-4">
        <div className="flex flex-col gap-3">
          <span className="w-max rounded-full bg-secondary px-3 py-1 font-medium text-[10px] text-muted-foreground uppercase tracking-[0.2em]">
            Library
          </span>
          <h2 className="font-semibold text-3xl tracking-tight sm:text-4xl" id="library-heading">
            Recently mapped
          </h2>
        </div>
        <Button
          aria-label="Reload recents"
          className="rounded-full"
          loading={library.isFetching}
          onClick={() => library.refetch()}
          size="icon"
          variant="ghost"
        >
          <ArrowClockwiseIcon weight="light" />
        </Button>
      </div>

      {library.isPending ? (
        <div className="grid grid-cols-1 gap-6 sm:grid-cols-2 xl:grid-cols-3">
          {Array.from({ length: 3 }, (_, i) => (
            <Skeleton className="aspect-[4/3] rounded-[1.75rem]" key={i} />
          ))}
        </div>
      ) : library.isError ? (
        <Alert variant="error">
          <AlertTitle>Couldn&apos;t load the library</AlertTitle>
          <AlertDescription>{library.error.message}</AlertDescription>
          <AlertAction>
            <Button onClick={() => library.refetch()} size="sm" variant="outline">
              Retry
            </Button>
          </AlertAction>
        </Alert>
      ) : books.length === 0 ? (
        <Empty className="rounded-[2rem] border border-dashed py-16">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <BooksIcon weight="light" />
            </EmptyMedia>
            <EmptyTitle>{library.data?.length ? "No saved books match" : "No saved books yet"}</EmptyTitle>
            <EmptyDescription>
              {library.data?.length
                ? "Change the filters, or pick Upload as the source to see saved EPUBs."
                : "Search the catalog and choose Schema, or drop an EPUB above."}
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <div className="grid grid-cols-1 gap-6 sm:grid-cols-2 xl:grid-cols-3">
          {books.map((record, index) => (
            <Reveal delay={Math.min(index, 6) * 0.06} key={record.id}>
              <BookCard now={now} record={record} />
            </Reveal>
          ))}
        </div>
      )}
    </section>
  );
}
