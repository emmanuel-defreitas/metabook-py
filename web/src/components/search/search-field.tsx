"use client";

import { useEffect, useRef } from "react";
import { MagnifyingGlassIcon } from "@phosphor-icons/react";
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group";
import { Kbd } from "@/components/ui/kbd";
import { cn } from "@/lib/utils";

/** Enter submits; there is deliberately no Search button (desktop parity). */
export function SearchField({
  value,
  onChange,
  onSubmit,
  size = "default",
  autoFocus,
  className,
}: {
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
  size?: "default" | "hero";
  autoFocus?: boolean;
  className?: string;
}) {
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        input.current?.focus();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);
  const hero = size === "hero";
  return (
    <form
      className={cn("w-full", className)}
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
      role="search"
    >
      <InputGroup
        className={cn(
          "rounded-lg",
          hero && "h-14 bg-card text-lg shadow-[0_18px_48px_-24px_rgba(15,18,23,0.28)] sm:h-16 sm:text-lg",
        )}
      >
        <InputGroupAddon className={cn(hero && "ps-5")}>
          <MagnifyingGlassIcon className={cn(hero && "size-5!")} weight="light" />
        </InputGroupAddon>
        <InputGroupInput
          ref={input}
          aria-keyshortcuts="Meta+K Control+K"
          aria-label="Search titles, authors, ISBNs or Gutenberg IDs"
          autoComplete="off"
          autoFocus={autoFocus}
          className={cn(hero && "text-lg")}
          enterKeyHint="search"
          onChange={(event) => onChange(event.target.value)}
          placeholder={hero ? "Search a title, author, ISBN or #Gutenberg ID" : "Search the catalog"}
          type="search"
          value={value}
        />
        <InputGroupAddon align="inline-end" className={cn("shrink-0 whitespace-nowrap max-sm:hidden [&>kbd]:shrink-0", hero && "pe-5")}>
          <Kbd title="⌘K on Mac · Ctrl+K on Windows/Linux">⌘/Ctrl K</Kbd>
          <Kbd>↵</Kbd>
        </InputGroupAddon>
      </InputGroup>
    </form>
  );
}
