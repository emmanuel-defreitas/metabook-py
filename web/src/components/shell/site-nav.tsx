import Link from "next/link";
import { Mark } from "@/components/brand/mark";
import { ThemeToggle } from "./theme-toggle";

/** Floating island nav, detached from the top edge. */
export function SiteNav() {
  return (
    <header className="pointer-events-none sticky top-0 z-40 flex justify-center px-4 pt-4 sm:pt-6">
      <nav
        aria-label="Primary"
        className="pointer-events-auto flex items-center gap-1 rounded-lg bg-background/70 p-1.5 shadow-[0_8px_32px_-12px_rgba(15,18,23,0.18)] ring-1 ring-foreground/8 backdrop-blur-xl dark:bg-card/70 dark:shadow-[0_8px_32px_-12px_rgba(0,0,0,0.6)]"
      >
        <Link
          className="group flex items-center gap-2.5 rounded-lg py-1 ps-1 pe-3 outline-none transition-colors duration-300 ease-spring hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring"
          href="/"
        >
          <Mark size={28} />
          <span className="font-semibold tracking-tight">Metabook</span>
        </Link>
        <span aria-hidden className="mx-1 h-5 w-px bg-border" />
        <Link
          className="rounded-lg px-3 py-1.5 text-muted-foreground text-sm outline-none transition-colors duration-300 ease-spring hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
          href="/library"
        >
          Library
        </Link>
        <Link
          className="rounded-lg px-3 py-1.5 text-muted-foreground text-sm outline-none transition-colors duration-300 ease-spring hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
          href="/documents"
        >
          Upload
        </Link>
        <ThemeToggle />
      </nav>
    </header>
  );
}
