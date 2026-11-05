"use client";

import { useState } from "react";
import { Recents } from "./recents";
import { CatalogSearch } from "@/components/search/catalog-search";
import { DEFAULT_FILTERS, type SearchFilters } from "@/lib/filters";

export function LibraryView() {
  const [filters, setFilters] = useState<SearchFilters>({ ...DEFAULT_FILTERS, source: "" });
  const [query, setQuery] = useState("");
  return <div className="mx-auto flex w-full max-w-6xl flex-col gap-10 px-4 py-16 sm:px-8">
    <h1 className="text-4xl font-semibold tracking-tight">Your library</h1>
    <CatalogSearch libraryOnly filters={filters} onFiltersChange={setFilters} onQueryChange={setQuery} />
    <Recents filters={filters} query={query} />
  </div>;
}
