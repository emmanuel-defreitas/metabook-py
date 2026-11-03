import { cn } from "@/lib/utils";

/** The metaBook mark on its green tile (from example/assets/icons/metabook-mark.svg). */
export function Mark({ className, size = 32 }: { className?: string; size?: number }) {
  return (
    <span
      aria-hidden
      className={cn(
        "inline-flex shrink-0 items-center justify-center rounded-[28%] bg-mark text-mark-foreground shadow-[inset_0_1px_1px_rgba(255,255,255,0.28),0_6px_16px_-6px_color-mix(in_oklab,var(--mark)_60%,transparent)]",
        className,
      )}
      style={{ width: size, height: size }}
    >
      <svg viewBox="0 0 48 48" fill="none" width={size * 0.62} height={size * 0.62}>
        <path
          d="M24 14 7 6v29l17 8 17-8V6l-17 8Zm0 0v29M13 17l7 3v15l-7-3V17Zm22 0-7 3v15l7-3V17Z"
          stroke="currentColor"
          strokeWidth={2.4}
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    </span>
  );
}
