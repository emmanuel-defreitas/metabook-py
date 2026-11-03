import { Suspense } from "react";
import { GutenbergBook } from "@/components/result/gutenberg-book";
import { ResultSkeleton } from "@/components/result/result-states";

export default function GutenbergBookPage({ params }: PageProps<"/gutenberg/[id]">) {
  return (
    <Suspense fallback={<ResultSkeleton />}>
      {params.then(({ id }) => (
        <GutenbergBook id={Number(id)} />
      ))}
    </Suspense>
  );
}
