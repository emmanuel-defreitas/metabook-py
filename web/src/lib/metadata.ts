/** Keep metadata truthful but omit absent and zero-valued display rows. */
export function isVisibleMetadataValue(value: unknown): boolean {
  if (value == null || value === false || value === 0) return false;
  if (typeof value === "number") return Number.isFinite(value);
  if (typeof value !== "string") return true;
  const text = value.trim();
  return text.length > 0 &&
    !/^(Not provided|Not applicable|Not requested|Author not provided|Size unavailable)/.test(text) &&
    !/^0(?:[.,]0+)?(?:\s|%|$)/.test(text);
}
