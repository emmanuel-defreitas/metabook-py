"use client";

import { useRouter } from "next/navigation";
import { useMemo, useState } from "react";
import { useLibrary } from "@/hooks/use-library";
import { categoryOptions, filtersToParams, type SearchFilters } from "@/lib/filters";
import { cn } from "@/lib/utils";
import { FilterBar } from "./filter-bar";
import { SearchField } from "./search-field";

/** Search field + shared filters. Enter submits an owned snapshot to /search. */
export function CatalogSearch({
  libraryOnly = false,
  initialQuery = "",
  filters,
  onFiltersChange,
  onQueryChange,
  size = "default",
  className,
}: {
  libraryOnly?: boolean;
  initialQuery?: string;
  filters: SearchFilters;
  onFiltersChange: (filters: SearchFilters) => void;
  onQueryChange?: (query: string) => void;
  size?: "default" | "hero";
  className?: string;
}) {
  const router = useRouter();
  const [query, setQuery] = useState(initialQuery);
  const library = useLibrary();
  const categories = useMemo(
    () => categoryOptions(library.data ?? [], filters.category),
    [library.data, filters.category],
  );

  const submit = () => {
    if (libraryOnly) return;
    const params = filtersToParams(filters);
    params.set("q", query.trim());
    router.push(`/search?${params}`);
  };

  return (
    <div className={cn("flex w-full flex-col gap-3", className)}>
      <SearchField
        autoFocus={size === "hero"}
        onChange={(value) => {
          setQuery(value);
          onQueryChange?.(value);
        }}
        onSubmit={submit}
        size={size}
        value={query}
      />
      <FilterBar
        libraryOnly={libraryOnly}
        categories={categories}
        className={cn(size === "hero" && "justify-center")}
        filters={filters}
        onChange={onFiltersChange}
      />
    </div>
  );
}
