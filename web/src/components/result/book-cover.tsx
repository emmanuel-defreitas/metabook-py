"use client";

import { BookOpenIcon } from "@phosphor-icons/react";
import Image from "next/image";
import { useState } from "react";
import { gutenbergCoverUrl } from "@/lib/api/client";

export function BookCover({ id, title, coverId }: { id?: number; title: string; coverId?: number | null }) {
  const [failed, setFailed] = useState(false);
  const url = id ? gutenbergCoverUrl(id) : coverId ? `https://covers.openlibrary.org/b/id/${coverId}-L.jpg?default=false` : null;
  return <div className="w-24 shrink-0 rounded-2xl bg-foreground/5 p-1.5 sm:w-32">
    <div className="relative aspect-[2/3] overflow-hidden rounded-xl bg-secondary outline-1 outline-black/10 dark:outline-white/10">
      {url && !failed ? <Image src={url} alt={`Cover of ${title}`} fill sizes="128px" unoptimized onError={() => setFailed(true)} className="object-cover" /> : <div className="flex h-full flex-col items-center justify-center gap-2 p-3 text-center text-muted-foreground"><BookOpenIcon size={32} weight="light" /><span className="text-xs">{title}</span></div>}
    </div>
  </div>;
}
