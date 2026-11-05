//! `metabook analyze <file>` — print a document's structure as JSON.
//!
//! Handles Markdown (.md, .markdown) and plain text (.txt, .text). HTML and
//! PDF need the Python adapters; use `POST /api/documents/analyze` for those.

use std::path::Path;
use std::process::ExitCode;

use metabook_core::{Format, analyze, decode};
use serde_json::json;

const USAGE: &str = "usage: metabook analyze <file.md|file.txt>";

fn format_for(path: &Path) -> Option<Format> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "md" | "markdown" => Some(Format::Markdown),
        "txt" | "text" => Some(Format::Text),
        _ => None,
    }
}

fn run(args: &[String]) -> Result<String, String> {
    let [cmd, file] = args else {
        return Err(USAGE.into());
    };
    if cmd != "analyze" {
        return Err(USAGE.into());
    }
    let path = Path::new(file);
    let format = format_for(path).ok_or_else(|| {
        format!("{file}: unsupported type (the CLI handles .md and .txt; use the API for HTML/PDF)")
    })?;
    let data = std::fs::read(path).map_err(|e| format!("{file}: {e}"))?;
    let text = decode(&data).map_err(|e| format!("{file}: {e}"))?;
    let structure = analyze(format, &text);
    let out = json!({
        "document": {
            "format": format.as_str(),
            "size_bytes": data.len(),
            "filename": path.file_name().and_then(|n| n.to_str()),
        },
        "structure": structure,
        "meta": { "schema_version": "1.0", "parser": "rust" },
    });
    serde_json::to_string_pretty(&out).map_err(|e| e.to_string())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::from(2)
        }
    }
}
