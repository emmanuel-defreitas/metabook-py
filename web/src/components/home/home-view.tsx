"use client";

import { LockKeyIcon, TreeStructureIcon, WaveformIcon } from "@phosphor-icons/react";
import { useState } from "react";
import { Mark } from "@/components/brand/mark";
import { DocumentView } from "@/components/documents/document-view";
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
    body: "Main content stays private. Only explicitly separated publication matter can be displayed.",
  },
];

export function HomeView() {
  const [filters, setFilters] = useState<SearchFilters>(DEFAULT_FILTERS);

  return (
    <div className="mx-auto flex w-full max-w-6xl flex-col gap-24 px-4 pt-16 pb-32 sm:px-8 sm:pt-24 lg:gap-32">
      <section className="flex flex-col items-center gap-10 text-center">
        <Reveal className="flex flex-col items-center gap-6">
          <Mark className="rounded-xl" size={64} />
          <span className="rounded-lg bg-secondary px-3 py-1 font-medium text-[10px] text-muted-foreground uppercase tracking-[0.2em] ring-1 ring-foreground/6">
            Structural metadata for books
          </span>
          <h1 className="max-w-3xl text-balance font-semibold text-5xl leading-[1.02] tracking-[-0.035em] sm:text-7xl">
            See how a book is built.
          </h1>
          <p className="max-w-xl text-balance text-lg text-muted-foreground">
            Search Project Gutenberg and OpenAlex, or upload a book or document. Metabook maps parts, chapters,
            paragraphs and sentences without exposing the main text.
          </p>
        </Reveal>
        <Reveal className="w-full max-w-3xl" delay={0.1}>
          <CatalogSearch
            filters={filters}
            onFiltersChange={setFilters}
            size="hero"
          />
          <p className="mt-4 text-subtle-foreground text-xs">
            Press Enter to search the selected catalog. Your saved books are in Library.
          </p>
        </Reveal>
      </section>

      <DocumentView embedded />
      <section>
        <div className="grid gap-3 md:grid-cols-3">
          {FACTS.map((fact, index) => (
            <Reveal delay={0.08 * (index + 1)} key={fact.title}>
              <div className="flex gap-4 rounded-xl p-4 ring-1 ring-foreground/6 transition-colors duration-500 ease-spring hover:bg-card">
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

    </div>
  );
}
