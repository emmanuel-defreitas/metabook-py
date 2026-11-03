export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || !Number.isFinite(bytes)) return "Size unavailable";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
}

const dateTime = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

export function formatTimestamp(value: string | number | null | undefined): string {
  if (value == null || value === "") return "Not provided";
  const time = typeof value === "number" ? value : Date.parse(value);
  return Number.isNaN(time) ? String(value) : dateTime.format(time);
}

const relative = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });

export function formatRelative(time: number | null, now = Date.now()): string {
  if (time == null) return "Saved date unavailable";
  const seconds = Math.round((time - now) / 1000);
  const steps: [Intl.RelativeTimeFormatUnit, number][] = [
    ["year", 31536000],
    ["month", 2592000],
    ["day", 86400],
    ["hour", 3600],
    ["minute", 60],
  ];
  for (const [unit, size] of steps) {
    if (Math.abs(seconds) >= size) return relative.format(Math.round(seconds / size), unit);
  }
  return "just now";
}

export function formatNumber(value: number | null | undefined, digits = 0): string {
  if (value == null || !Number.isFinite(value)) return "Not provided";
  return value.toLocaleString(undefined, { maximumFractionDigits: digits });
}

export function schemaLabel(schema: string | null | undefined): string {
  if (!schema) return "Unknown schema";
  return schema.replace(/_/g, " ").replace(/^\w/, (c) => c.toUpperCase());
}

export function authorNames(authors: { name: string }[] | string[] | undefined): string {
  const names = (authors ?? [])
    .map((author) => (typeof author === "string" ? author : author?.name))
    .filter(Boolean);
  return names.length ? names.join(", ") : "Author not provided";
}
