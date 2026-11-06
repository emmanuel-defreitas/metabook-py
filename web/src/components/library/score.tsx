"use client";

import { Tooltip, TooltipTrigger, TooltipPopup } from "@/components/ui/tooltip";
import { Meter, MeterIndicator, MeterLabel, MeterTrack, MeterValue } from "@/components/ui/meter";

/**
 * Heuristic schema-support score saved with a scan. Not calibrated confidence;
 * a missing score stays explicit instead of showing 0%.
 */
export function SchemaScore({ score, compact }: { score: number | null | undefined; compact?: boolean }) {
  if (score == null || !Number.isFinite(score)) {
    return <span className="text-subtle-foreground text-xs">Score unavailable</span>;
  }
  return (
    <Tooltip>
    <TooltipTrigger render={<div tabIndex={0} className="w-full outline-none focus-visible:ring-2 focus-visible:ring-ring" />} aria-label="About schema support">
    <Meter
      className={compact ? "w-24 gap-1" : "w-full gap-1.5"}
      format={{ style: "percent", maximumFractionDigits: 0 }}
      max={1}
      min={0}
      value={Math.min(1, Math.max(0, score))}
    >
      <div className="flex items-center justify-between gap-2 text-xs">
        <MeterLabel className={compact ? "sr-only" : "font-normal text-muted-foreground text-xs"}>
          Schema support
        </MeterLabel>
        <MeterValue className="font-medium text-xs tabular-nums" />
      </div>
      <MeterTrack className="h-1 rounded-lg">
        <MeterIndicator className="bg-mark" />
      </MeterTrack>
    </Meter>
    </TooltipTrigger>
    <TooltipPopup className="max-w-72">
      <p className="p-2 leading-relaxed">Schema support measures how strongly structural clues, such as chapter headings and verse markers, support the detected book layout. {Math.round(score * 100)}% is a heuristic score, not the percentage parsed or a probability of correctness.</p>
    </TooltipPopup>
    </Tooltip>
  );
}
