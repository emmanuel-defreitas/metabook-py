"use client";

import { BookIcon } from "@phosphor-icons/react";
import Image from "next/image";
import Link from "next/link";
import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import { gutenbergCoverUrl } from "@/lib/api/client";
import type { UploadRecord } from "@/lib/api/types";
import { recordedActivity } from "@/lib/filters";
import { authorNames, formatBytes, formatRelative, schemaLabel } from "@/lib/format";
import { SchemaScore } from "./score";

function Cover({ record }: { record: UploadRecord }) {
  const [failed, setFailed] = useState(false);
  const url = record.gutenberg_id ? gutenbergCoverUrl(record.gutenberg_id) : record.book.open_library?.cover_id ? `https://covers.openlibrary.org/b/id/${record.book.open_library.cover_id}-L.jpg?default=false` : null;
  return (
    <div className="relative aspect-[2/3] w-full overflow-hidden rounded-xl bg-secondary ring-1 ring-foreground/6">
      {url && !failed ? (
        // Remote Gutenberg covers are served as-is, not through the image optimiser.
        <Image
          alt=""
          className="object-cover transition-transform duration-700 ease-spring group-hover:scale-[1.03]"
          fill
          onError={() => setFailed(true)}
          sizes="(min-width: 1280px) 12rem, (min-width: 640px) 25vw, 50vw"
          src={url}
          unoptimized
        />
      ) : (
        <div className="flex size-full items-center justify-center">
          <BookIcon className="size-10 text-subtle-foreground" weight="thin" />
        </div>
      )}
    </div>
  );
}

export function BookCard({ record, now }: { record: UploadRecord; now: number }) {
  const size = record.size_bytes ?? record.blob?.size_bytes;
  const source = record.source === "gutenberg" ? "Gutendex" : record.kind === "document" ? record.format.toUpperCase() : "Upload";
  return (
    <Link
      className="group block rounded-xl bg-foreground/[0.03] p-1.5 outline-none ring-1 ring-foreground/6 transition-[scale,box-shadow] duration-500 ease-spring hover:shadow-[0_24px_48px_-28px_rgba(15,18,23,0.35)] focus-visible:ring-2 focus-visible:ring-ring active:scale-[0.99] dark:bg-white/[0.03]"
      href={`/books/${encodeURIComponent(record.id)}`}
    >
      <article className="grid h-full grid-cols-2 gap-4 rounded-xl bg-card p-1.5 pe-4 shadow-[inset_0_1px_1px_rgba(255,255,255,0.6)] dark:shadow-[inset_0_1px_1px_rgba(255,255,255,0.05)]">
        <Cover record={record} />
        <div className="flex min-w-0 flex-col gap-3 py-3">
          <div className="flex flex-wrap gap-1">
            <Badge variant="outline">{source}</Badge>
            {record.gutenberg_id && <Badge variant="secondary">#{record.gutenberg_id}</Badge>}
          </div>
          <div className="flex min-w-0 flex-col gap-1">
            <h3 className="line-clamp-3 font-semibold leading-snug tracking-tight">
              {record.book?.title || "Untitled"}
            </h3>
            <p className="line-clamp-2 text-muted-foreground text-sm">{authorNames(record.book?.authors)}</p>
          </div>
          <dl className="mt-auto flex flex-col gap-1 text-subtle-foreground text-xs">
            <div className="truncate">
              <dt className="sr-only">Schema</dt>
              <dd>{schemaLabel(record.scan?.schema)}</dd>
            </div>
            <div>
              <dt className="sr-only">Size</dt>
              <dd>{size != null ? formatBytes(size) : "Size unavailable"}</dd>
            </div>
            <div>
              <dt className="sr-only">Changed</dt>
              <dd>{formatRelative(recordedActivity(record, now), now)}</dd>
            </div>
          </dl>
          <SchemaScore score={record.scan?.schema_score} />
        </div>
      </article>
    </Link>
  );
}
