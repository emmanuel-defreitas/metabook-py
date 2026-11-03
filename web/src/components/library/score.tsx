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
      <MeterTrack className="h-1 rounded-full">
        <MeterIndicator className="bg-mark" />
      </MeterTrack>
    </Meter>
  );
}
