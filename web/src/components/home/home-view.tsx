"use client";

import { ArrowRightIcon, LockKeyIcon, TreeStructureIcon, WaveformIcon } from "@phosphor-icons/react";
import Link from "next/link";
import { useState } from "react";
import { Mark } from "@/components/brand/mark";
import { DropZone } from "@/components/library/drop-zone";
import { Recents } from "@/components/library/recents";
import { Reveal } from "@/components/motion/reveal";
import { CatalogSearch } from "@/components/search/catalog-search";
import { DEFAULT_FILTERS, type SearchFilters } from "@/lib/filters";

const FACTS = [
  {
    icon: TreeStructureIcon,
    title: "Schema detection",
    body: "Scripture, sectioned books, collections or flat prose, scored on structural evidence.",
  },
  {
    icon: WaveformIcon,
    title: "Down to the clause",
    body: "Sentence and clause counts with bert-base-uncased token totals at every level.",
  },
  {
    icon: LockKeyIcon,
    title: "Text stays private",
    body: "Results carry positions and counts only. No prose is ever returned or stored.",
  },
];

export function HomeView() {
  const [filters, setFilters] = useState<SearchFilters>(DEFAULT_FILTERS);
  const [query, setQuery] = useState("");

  return (
    <div className="mx-auto flex w-full max-w-6xl flex-col gap-24 px-4 pt-16 pb-32 sm:px-8 sm:pt-24 lg:gap-32">
      <section className="flex flex-col items-center gap-10 text-center">
        <Reveal className="flex flex-col items-center gap-6">
          <Mark className="rounded-[30%]" size={64} />
          <span className="rounded-full bg-secondary px-3 py-1 font-medium text-[10px] text-muted-foreground uppercase tracking-[0.2em] ring-1 ring-foreground/6">
            Structural metadata for books
          </span>
          <h1 className="max-w-3xl text-balance font-semibold text-5xl leading-[1.02] tracking-[-0.035em] sm:text-7xl">
            See how a book is built.
          </h1>
          <p className="max-w-xl text-balance text-lg text-muted-foreground">
            Search Project Gutenberg and OpenAlex, or drop in an EPUB. Metabook maps parts, chapters,
            paragraphs and sentences without exposing a single line of text.
          </p>
        </Reveal>
        <Reveal className="w-full max-w-3xl" delay={0.1}>
          <CatalogSearch
            filters={filters}
            onFiltersChange={setFilters}
            onQueryChange={setQuery}
            size="hero"
          />
          <p className="mt-4 text-subtle-foreground text-xs">
            Press Enter to search the selected catalog. Typing also filters your saved library below.
          </p>
        </Reveal>
      </section>

      <section className="grid grid-cols-1 gap-6 lg:grid-cols-12">
        <Reveal className="flex flex-col gap-3 lg:col-span-7">
          <DropZone />
          <Link
            className="group flex w-max items-center gap-2 self-center rounded-full px-3 py-1.5 text-muted-foreground text-sm outline-none transition-colors duration-300 ease-spring hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
            href="/documents"
          >
            Not a book? Outline an HTML, Markdown, PDF or text document
            <ArrowRightIcon className="transition-transform duration-500 ease-spring group-hover:translate-x-0.5" weight="light" />
          </Link>
        </Reveal>
        <div className="flex flex-col gap-3 lg:col-span-5">
          {FACTS.map((fact, index) => (
            <Reveal delay={0.08 * (index + 1)} key={fact.title}>
              <div className="flex gap-4 rounded-[1.5rem] p-4 ring-1 ring-foreground/6 transition-colors duration-500 ease-spring hover:bg-card">
                <span className="flex size-10 shrink-0 items-center justify-center rounded-xl bg-secondary">
                  <fact.icon className="size-5 text-foreground" weight="light" />
                </span>
                <div className="flex flex-col gap-1">
                  <h3 className="font-medium tracking-tight">{fact.title}</h3>
                  <p className="text-muted-foreground text-sm">{fact.body}</p>
                </div>
              </div>
            </Reveal>
          ))}
        </div>
      </section>

      <Recents filters={filters} query={query} />
    </div>
  );
}
