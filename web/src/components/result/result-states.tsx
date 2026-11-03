"use client";

import { WarningCircleIcon } from "@phosphor-icons/react";
import Link from "next/link";
import { Button } from "@/components/ui/button";
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";

export function ResultSkeleton({ message }: { message?: string }) {
  return (
    <div className="mx-auto flex w-full max-w-6xl flex-col gap-10 px-4 pt-10 pb-32 sm:px-8 sm:pt-14" aria-busy>
      <div className="flex flex-col gap-4">
        <Skeleton className="h-5 w-20 rounded-full" />
        <Skeleton className="h-12 w-3/4 rounded-2xl" />
        <Skeleton className="h-6 w-1/3 rounded-full" />
        {message && (
          <p className="flex items-center gap-2 text-muted-foreground text-sm" aria-live="polite">
            <Spinner className="size-4" />
            {message}
          </p>
        )}
      </div>
      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        {Array.from({ length: 4 }, (_, i) => (
          <Skeleton className="h-20 rounded-2xl" key={i} />
        ))}
      </div>
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-12">
        <Skeleton className="aspect-square rounded-[2rem] lg:col-span-7" />
        <Skeleton className="h-96 rounded-[2rem] lg:col-span-5" />
      </div>
    </div>
  );
}

export function ResultError({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <div className="mx-auto w-full max-w-xl px-4 pt-24 pb-32">
      <Empty>
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <WarningCircleIcon weight="light" />
          </EmptyMedia>
          <EmptyTitle>Couldn&apos;t open this book</EmptyTitle>
          <EmptyDescription>{message}</EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <div className="flex gap-2">
            <Button className="rounded-full" render={<Link href="/#library" />} variant="outline">
              Return to library
            </Button>
            {onRetry && (
              <Button className="rounded-full" onClick={onRetry}>
                Try again
              </Button>
            )}
          </div>
        </EmptyContent>
      </Empty>
    </div>
  );
}
