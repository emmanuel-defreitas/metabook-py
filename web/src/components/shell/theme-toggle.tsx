"use client";

import { MoonIcon, SunIcon } from "@phosphor-icons/react";
import { useTheme } from "next-themes";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipPopup, TooltipTrigger } from "@/components/ui/tooltip";

export function ThemeToggle() {
  const { resolvedTheme, setTheme } = useTheme();
  // The resolved theme is unknown during SSR, so the label stays theme-neutral.
  const next = resolvedTheme === "dark" ? "light" : "dark";
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button
            aria-label="Toggle theme"
            className="rounded-lg"
            onClick={() => setTheme(next)}
            size="icon"
            variant="ghost"
          />
        }
      >
        <SunIcon className="hidden dark:block" weight="light" />
        <MoonIcon className="dark:hidden" weight="light" />
      </TooltipTrigger>
      <TooltipPopup>Toggle theme</TooltipPopup>
    </Tooltip>
  );
}
