// Browser client for the Book Structure API, served from the same origin
// under /api (Vercel services in production, a Next.js rewrite in dev).

import type { Analysis, CatalogPage, DocumentAnalysis, UploadRecord } from "./types";

const BASE = (process.env.NEXT_PUBLIC_API_BASE ?? "").replace(/\/$/, "");

/** Scan settings shared with the desktop client. */
export const SCAN_DETAIL = "sentence";
export const SCAN_TOKENIZER = "bert-base-uncased";

export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

type Detail = Record<string, unknown> | { msg?: unknown }[] | string | null | undefined;

/** Human-readable message for an API error body (mirrors the desktop app). */
export function describeError(status: number, detail: Detail): string {
  const obj: Record<string, unknown> =
    typeof detail === "object" && detail && !Array.isArray(detail) ? detail : {};
  const kind = (typeof detail === "string" ? detail : (obj.error as string)) ?? "";
  const message = typeof obj.message === "string" ? obj.message : undefined;
  switch (kind) {
    case "book_not_found":
      return "No book matched that search. Check the spelling or try an ISBN.";
    case "text_unavailable":
      return "The book exists but its text couldn't be retrieved from Project Gutenberg.";
    case "unsupported_format":
      return "This Gutendex book has no text or HTML format available for analysis.";
    case "invalid_epub":
    case "invalid_file":
      return `Invalid EPUB: ${message ?? "The file isn't a valid EPUB."}`;
    case "file_too_large":
      return typeof obj.max_bytes === "number"
        ? `That file exceeds the API's ${Math.round(obj.max_bytes / 1024 / 1024)} MB limit.`
        : "That file exceeds the API's upload size limit.";
    case "unsupported_type":
      return "That file type isn't supported. Use HTML, Markdown, PDF or plain text.";
    case "invalid_document":
      return `The document couldn't be parsed: ${message ?? "it may be damaged or empty."}`;
    case "url_not_allowed":
      return `That URL can't be fetched: ${message ?? "only public http(s) addresses are allowed."}`;
    case "fetch_timeout":
      return "The page took longer than 10 seconds to download. Try again or upload the file instead.";
    case "fetch_failed":
      return `The URL couldn't be downloaded: ${message ?? "the server returned an error."}`;
    case "database_unavailable":
      return "The API's database is unavailable right now. Try again shortly.";
    case "tokenizer_not_found":
      return `Tokenizer “${obj.tokenizer ?? "that tokenizer"}” wasn't found on the Hugging Face Hub.`;
    case "tokenizer_unavailable":
      return `“${obj.tokenizer ?? "The tokenizer"}” couldn't be fetched from the Hugging Face Hub right now. Try again shortly.`;
    case "blob_upload_failed":
      return "The API couldn't store the file. Check its BLOB_READ_WRITE_TOKEN configuration.";
    case "saved_result_not_found":
      return "That saved result no longer exists.";
  }
  // FastAPI request validation: [{loc, msg, type}, ...]
  if (Array.isArray(detail) && typeof detail[0]?.msg === "string") {
    return `The request was rejected: ${detail[0].msg}.`;
  }
  if (status === 404 && !kind) {
    return "This API deployment doesn't provide that endpoint yet.";
  }
  if (status === 413) return "That file is larger than the host accepts for uploads.";
  if (status === 422 && typeof detail === "string" && detail.trim()) return detail;
  if ([429, 502, 504].includes(status) && message) return message;
  return `The API returned an error (HTTP ${status}).`;
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${BASE}${path}`, {
      ...init,
      headers: { Accept: "application/json", ...init?.headers },
    });
  } catch (error) {
    if (init?.signal?.aborted) throw error;
    throw new ApiError("Couldn't reach the API. Check your connection and try again.", 0);
  }
  const text = await response.text();
  let body: unknown = null;
  try {
    body = text ? JSON.parse(text) : null;
  } catch {
    if (response.ok) throw new ApiError("The API returned invalid JSON.", response.status);
  }
  if (!response.ok) {
    const detail = (body as { detail?: Detail } | null)?.detail;
    throw new ApiError(describeError(response.status, detail), response.status);
  }
  return body as T;
}

function validAnalysis(value: Analysis): Analysis {
  if (
    !value?.book?.title?.trim() ||
    !value.structure?.schema?.trim() ||
    !Array.isArray(value.structure.nodes)
  ) {
    throw new ApiError(
      "The API response is missing a valid book or structural schema. Please try the book again.",
      200,
    );
  }
  return value;
}

export type SearchInput =
  | { kind: "title"; value: string }
  | { kind: "isbn"; value: string }
  | { kind: "gutenberg"; value: number };

/** One field accepts titles, hyphenated ISBNs, and `#1342`-style Gutenberg IDs. */
export function parseSearchInput(input: string): SearchInput {
  const trimmed = input.trim();
  const compact = trimmed.replace(/[\s-]/g, "");
  if (/^(\d{13}|\d{9}[\dXx])$/.test(compact)) {
    return { kind: "isbn", value: compact.toUpperCase() };
  }
  const id = trimmed.replace(/^#+/, "");
  if (/^\d+$/.test(id) && Number(id) > 0) {
    return { kind: "gutenberg", value: Number(id) };
  }
  return { kind: "title", value: trimmed };
}

export interface CatalogQuery {
  q: string;
  page: number;
  date: string[];
  category: string[];
  kind: string[];
  source: string;
}

export function catalogSearch(query: CatalogQuery, signal?: AbortSignal): Promise<CatalogPage> {
  const params = new URLSearchParams();
  const input = parseSearchInput(query.q);
  if (input.kind === "title") params.set("q", input.value);
  if (input.kind === "isbn") params.set("isbn", input.value);
  if (input.kind === "gutenberg") params.set("gutenberg_id", String(input.value));
  params.set("page", String(query.page));
  for (const date of query.date) params.append("date", date);
  for (const category of query.category) params.append("category", category);
  for (const kind of query.kind) params.append("kind", kind);
  if (query.source) params.append("source", query.source);
  return request<CatalogPage>(`/api/books/catalog/search?${params}`, { signal });
}

export function listLibrary(signal?: AbortSignal): Promise<UploadRecord[]> {
  return request<UploadRecord[]>("/api/books/uploads?limit=200", { signal });
}

export async function openSaved(id: string, signal?: AbortSignal): Promise<Analysis> {
  return validAnalysis(
    await request<Analysis>(`/api/books/uploads/${encodeURIComponent(id)}/structure`, { signal }),
  );
}

export async function analyseGutenberg(id: number, signal?: AbortSignal): Promise<Analysis> {
  const params = new URLSearchParams({
    gutenberg_id: String(id),
    include_paragraphs: "true",
    detail: SCAN_DETAIL,
    tokenizer: SCAN_TOKENIZER,
    language: "",
  });
  return validAnalysis(await request<Analysis>(`/api/books/structure?${params}`, { signal }));
}

export interface UploadProgress {
  loaded: number;
  total: number;
}

/** Multipart upload over XHR, so the real request bytes can be reported. */
function uploadFile<T>(
  path: string,
  file: File,
  onProgress: (progress: UploadProgress) => void,
  validate: (body: T) => T,
): { promise: Promise<T>; abort: () => void } {
  const xhr = new XMLHttpRequest();
  const promise = new Promise<T>((resolve, reject) => {
    xhr.open("POST", `${BASE}${path}`);
    xhr.setRequestHeader("Accept", "application/json");
    xhr.upload.onprogress = (event) => {
      if (event.lengthComputable) onProgress({ loaded: event.loaded, total: event.total });
    };
    xhr.onerror = () =>
      reject(new ApiError("Couldn't reach the API. Check your connection and try again.", 0));
    xhr.onabort = () => reject(new ApiError("Upload cancelled.", 0));
    xhr.onload = () => {
      let body: unknown = null;
      try {
        body = xhr.responseText ? JSON.parse(xhr.responseText) : null;
      } catch {
        // handled below
      }
      if (xhr.status >= 200 && xhr.status < 300) {
        try {
          resolve(validate(body as T));
        } catch (error) {
          reject(error);
        }
        return;
      }
      const detail = (body as { detail?: Detail } | null)?.detail;
      reject(new ApiError(describeError(xhr.status, detail), xhr.status));
    };
    const form = new FormData();
    form.append("file", file, file.name);
    xhr.send(form);
  });
  return { promise, abort: () => xhr.abort() };
}

export function uploadEpub(file: File, onProgress: (progress: UploadProgress) => void) {
  const params = new URLSearchParams({
    include_paragraphs: "true",
    detail: SCAN_DETAIL,
    tokenizer: SCAN_TOKENIZER,
  });
  return uploadFile<Analysis>(`/api/books/upload?${params}`, file, onProgress, validAnalysis);
}

// ── Single documents ───────────────────────────────────────────────────────────

/** Extensions the document endpoints accept (docs/single-document-schema.md). */
export const DOCUMENT_ACCEPT =
  ".html,.htm,.xhtml,.md,.markdown,.pdf,.txt,.text,text/html,application/xhtml+xml,text/markdown,application/pdf,text/plain";

function validDocument(value: DocumentAnalysis): DocumentAnalysis {
  if (!value?.document?.format || !Array.isArray(value.structure?.sections)) {
    throw new ApiError("The API response is missing a document structure.", 200);
  }
  return value;
}

export function uploadDocument(file: File, onProgress: (progress: UploadProgress) => void) {
  return uploadFile<DocumentAnalysis>("/api/documents/analyze", file, onProgress, validDocument);
}

export async function analyseDocumentUrl(url: string, signal?: AbortSignal): Promise<DocumentAnalysis> {
  return validDocument(
    await request<DocumentAnalysis>("/api/documents/analyze-url", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ url }),
      signal,
    }),
  );
}

export function gutenbergCoverUrl(id: number): string {
  return `https://www.gutenberg.org/cache/epub/${id}/pg${id}.cover.medium.jpg`;
}

export function openAlexUrl(id: string): string | null {
  return /^W\d+$/.test(id) ? `https://openalex.org/${id}` : null;
}
