//! Blocking client for the Book Structure API.
//!
//! Runs on GPUI's background executor, so plain blocking I/O (ureq) is fine
//! here — no async runtime needed. Every public function returns either a
//! ready-to-display [`Analysis`] or a user-facing error message.

use std::io::Write as _;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

const TIMEOUT: Duration = Duration::from_secs(120);

/// A successful structural analysis, ready for presentation.
pub struct Analysis {
    pub title: String,
    /// Pretty-printed JSON of the full API response.
    pub schema_json: String,
}

/// GET /api/books/structure — search Project Gutenberg by title/author or ISBN.
pub fn search(base: &str, query: &str, isbn: &str) -> Result<Analysis, String> {
    let mut request = ureq::get(&format!("{base}/api/books/structure"))
        .query("include_paragraphs", "false")
        .timeout(TIMEOUT);
    if !query.is_empty() {
        request = request.query("title", query);
    }
    if !isbn.is_empty() {
        request = request.query("isbn", isbn);
    }

    match request.call() {
        // ureq treats 3xx as success; the API uses 300 for "multiple matches".
        Ok(resp) if resp.status() == 300 => Err(disambiguation_message(resp)),
        Ok(resp) => parse_analysis(resp),
        Err(ureq::Error::Status(code, resp)) => Err(status_message(code, resp)),
        Err(err) => Err(unreachable_message(base, &err)),
    }
}

/// POST /api/books/upload — upload an EPUB file for analysis.
pub fn upload(base: &str, path: &Path) -> Result<Analysis, String> {
    let bytes = std::fs::read(path)
        .map_err(|err| format!("Couldn't read “{}”: {err}", path.display()))?;
    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "book.epub".into());

    let (body, content_type) = multipart_body(&filename, &bytes);

    let result = ureq::post(&format!("{base}/api/books/upload"))
        .query("include_paragraphs", "false")
        .set("Content-Type", &content_type)
        .timeout(TIMEOUT)
        .send_bytes(&body);

    match result {
        Ok(resp) => parse_analysis(resp),
        Err(ureq::Error::Status(code, resp)) => Err(status_message(code, resp)),
        Err(err) => Err(unreachable_message(base, &err)),
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn multipart_body(filename: &str, bytes: &[u8]) -> (Vec<u8>, String) {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let boundary = format!("----metabook-{nonce}");

    let mut body = Vec::with_capacity(bytes.len() + 512);
    let _ = write!(
        body,
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: application/epub+zip\r\n\r\n",
    );
    body.extend_from_slice(bytes);
    let _ = write!(body, "\r\n--{boundary}--\r\n");

    let content_type = format!("multipart/form-data; boundary={boundary}");
    (body, content_type)
}

fn parse_analysis(resp: ureq::Response) -> Result<Analysis, String> {
    let text = resp
        .into_string()
        .map_err(|err| format!("Couldn't read the API response: {err}"))?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|err| format!("The API returned invalid JSON: {err}"))?;

    let title = value["book"]["title"]
        .as_str()
        .unwrap_or("Untitled")
        .to_string();
    let schema_json = serde_json::to_string_pretty(&value).unwrap_or(text);

    Ok(Analysis { title, schema_json })
}

fn disambiguation_message(resp: ureq::Response) -> String {
    let Ok(value) = resp
        .into_string()
        .map_err(|_| ())
        .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|_| ()))
    else {
        return "Multiple books matched. Try a more specific title or an ISBN.".into();
    };

    let mut message = String::from("Multiple books matched:\n");
    if let Some(matches) = value["matches"].as_array() {
        for entry in matches.iter().take(5) {
            let title = entry["title"].as_str().unwrap_or("?");
            let authors = entry["authors"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            message.push_str(&format!("• {title} — {authors}\n"));
        }
    }
    message.push_str("Try a more specific title or an ISBN.");
    message
}

fn status_message(code: u16, resp: ureq::Response) -> String {
    let detail = resp
        .into_string()
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .map(|value| value["detail"].clone())
        .unwrap_or(Value::Null);

    let error_kind = detail["error"]
        .as_str()
        .or_else(|| detail.as_str())
        .unwrap_or("")
        .to_string();

    match error_kind.as_str() {
        "book_not_found" => "No book matched that search. Check the spelling or try an ISBN.".into(),
        "text_unavailable" => {
            "The book exists but its text couldn't be retrieved from Project Gutenberg.".into()
        }
        "invalid_epub" | "invalid_file" => {
            let hint = detail["message"].as_str().unwrap_or("The file isn't a valid EPUB.");
            format!("Invalid EPUB: {hint}")
        }
        "file_too_large" => "That EPUB exceeds the API's upload size limit.".into(),
        "blob_upload_failed" => {
            "The API couldn't store the file. Check its BLOB_READ_WRITE_TOKEN configuration.".into()
        }
        _ => format!("The API returned an error (HTTP {code})."),
    }
}

fn unreachable_message(base: &str, err: &ureq::Error) -> String {
    format!("Couldn't reach the API at {base}. Start it with `make dev`, then try again. ({err})")
}
