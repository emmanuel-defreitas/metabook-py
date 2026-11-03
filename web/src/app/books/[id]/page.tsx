import { Suspense } from "react";
import { ResultSkeleton } from "@/components/result/result-states";
import { SavedBook } from "@/components/result/saved-book";

export default function SavedBookPage({ params }: PageProps<"/books/[id]">) {
  return (
    <Suspense fallback={<ResultSkeleton />}>
      {params.then(({ id }) => (
        <SavedBook id={decodeURIComponent(id)} />
      ))}
    </Suspense>
  );
}
