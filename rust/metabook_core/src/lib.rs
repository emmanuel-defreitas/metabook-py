//! metabook_core — native parsing core for metabook-py.
//!
//! Ports of the Python reference implementation, kept output-identical (the
//! Python test suite runs parity checks against every fixture):
//! - `textstats`: sentence splitting and word counting shared by the book
//!   counter and single-document analysis.
//! - `builder`: block tree → section tree with counts.
//! - `text_adapter`, `markdown_adapter`: plain text and Markdown → blocks.
//!
//! HTML and PDF adapters stay in Python; their blocks are handed to
//! `build_structure` through the Python bindings.

pub mod blocks;
pub mod builder;
pub mod markdown_adapter;
mod pystr;
pub mod text_adapter;
pub mod textstats;

#[cfg(feature = "python")]
mod python;

pub use builder::{Structure, build_structure};

/// Formats the native core can parse end to end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Text,
}

impl Format {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "markdown" => Some(Self::Markdown),
            "text" => Some(Self::Text),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Text => "text",
        }
    }
}

/// Parse and build the section tree for a text or Markdown document.
pub fn analyze(format: Format, text: &str) -> Structure {
    let blocks = match format {
        Format::Markdown => markdown_adapter::parse_markdown(text),
        Format::Text => text_adapter::parse_text(text),
    };
    build_structure(blocks)
}

/// Decode document bytes like the Python service: strip a UTF-8 BOM, reject
/// NUL bytes in the first 4 KiB, then UTF-8 with a Latin-1 fallback.
pub fn decode(data: &[u8]) -> Result<String, &'static str> {
    let data = data.strip_prefix(b"\xef\xbb\xbf").unwrap_or(data);
    if data[..data.len().min(4096)].contains(&0) {
        return Err("binary content in a text document");
    }
    Ok(match std::str::from_utf8(data) {
        Ok(s) => s.to_string(),
        Err(_) => data.iter().map(|&b| b as char).collect(),
    })
}
