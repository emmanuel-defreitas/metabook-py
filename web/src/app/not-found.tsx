import Link from "next/link";
import { Button } from "@/components/ui/button";

export default function NotFound() {
  return (
    <div className="mx-auto flex max-w-md flex-col items-center gap-4 px-4 pt-32 text-center">
      <span className="rounded-full bg-secondary px-3 py-1 font-medium text-[10px] text-muted-foreground uppercase tracking-[0.2em]">
        404
      </span>
      <h1 className="font-semibold text-3xl tracking-tight">This page has no structure.</h1>
      <Button className="rounded-full" render={<Link href="/" />} variant="outline">
        Back to Metabook
      </Button>
    </div>
  );
}
