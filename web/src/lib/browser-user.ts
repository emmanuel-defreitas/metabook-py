/** Anonymous library identity. Never send this UUID to third-party services. */
const KEY = "metabook.user-id";
const UUID_V4 = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
let temporaryId: string | undefined;

export function browserUserId(): string {
  if (typeof window === "undefined") throw new Error("Browser identity requires a browser");
  try {
    const existing = window.localStorage.getItem(KEY);
    if (existing && UUID_V4.test(existing)) return existing;
    const id = temporaryId ?? crypto.randomUUID();
    window.localStorage.setItem(KEY, id);
    return id;
  } catch {
    // Restricted storage still gets an isolated library for this page session.
    temporaryId ??= crypto.randomUUID();
    return temporaryId;
  }
}
