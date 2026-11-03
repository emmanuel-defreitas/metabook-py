import type { Metadata } from "next";
import { Suspense } from "react";
import { SearchResults, SearchResultsSkeleton } from "@/components/search/search-results";

export const metadata: Metadata = { title: "Search" };

export default function SearchPage() {
  return (
    <Suspense fallback={<SearchResultsSkeleton />}>
      <SearchResults />
    </Suspense>
  );
}
