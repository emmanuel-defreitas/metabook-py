//! Blocking client for the Book Structure API.
//!
//! Runs on GPUI's background executor, so plain blocking I/O (ureq) is fine
//! here — no async runtime needed. Every public function returns either a
//! ready-to-display [`Analysis`] or a user-facing error message.

use std::collections::HashMap;
use std::io::{Read as _, Write as _};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

const TIMEOUT: Duration = Duration::from_secs(120);

/// ureq's `into_string()` refuses bodies over 10 MB, which deep `detail=`
/// levels on large books exceed easily; read manually with a far larger cap.
const MAX_BODY_BYTES: u64 = 200 * 1024 * 1024;

/// Read the full response body, replacing `into_string()`'s 10 MB cap.
fn read_body(resp: ureq::Response) -> Result<String, String> {
    let mut text = String::new();
    resp.into_reader()
        .take(MAX_BODY_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|err| format!("Couldn't read the API response: {err}"))?;
    if text.len() as u64 > MAX_BODY_BYTES {
        return Err(
            "The response is too large to display (over 200 MB). \
             Try a shallower detail level, like Sentence or Paragraph."
                .into(),
        );
    }
    Ok(text)
}

/// A successful structural analysis, ready for presentation.
pub struct Analysis {
    pub title: String,
    /// Pretty-printed JSON of the full API response.
    pub schema_json: String,
    /// The structure nodes as a plain label tree (no book text).
    pub tree: Vec<TreeNode>,
    /// Node id → (first, last) 0-based line of that node in `schema_json`.
    pub ranges: HashMap<String, (usize, usize)>,
}

/// One node of the structural tree: a part/book, chapter, or verse/paragraph.
pub struct TreeNode {
    /// Stable positional id: "n{i}" for top-level, then ".{j}" per child level.
    pub id: String,
    pub label: String,
    pub children: Vec<TreeNode>,
}

/// One candidate from a search that matched several books.
#[derive(Clone)]
pub struct BookMatch {
    pub gutenberg_id: u64,
    pub title: String,
    pub authors: String,
    pub language: String,
}

/// A search either resolves to one analysed book or to a list to choose from.
pub enum SearchOutcome {
    Analysis(Analysis),
    Matches(Vec<BookMatch>),
}

/// GET /api/books/structure — search Project Gutenberg by title/author or ISBN.
pub fn search(base: &str, query: &str, isbn: &str, detail: &str) -> Result<SearchOutcome, String> {
    let mut request = ureq::get(&format!("{base}/api/books/structure"))
        .query("include_paragraphs", "true")
        .query("detail", detail)
        .timeout(TIMEOUT);
    if !query.is_empty() {
        request = request.query("title", query);
    }
    if !isbn.is_empty() {
        request = request.query("isbn", isbn);
    }

    match request.call() {
        // ureq treats 3xx as success; the API uses 300 for "multiple matches".
        Ok(resp) if resp.status() == 300 => parse_matches(resp).map(SearchOutcome::Matches),
        Ok(resp) => parse_analysis(resp).map(SearchOutcome::Analysis),
        Err(ureq::Error::Status(code, resp)) => Err(status_message(code, resp)),
        Err(err) => Err(unreachable_message(base, &err)),
    }
}

/// GET /api/books/structure?gutenberg_id=… — analyse one selected match.
pub fn fetch_by_id(base: &str, gutenberg_id: u64, detail: &str) -> Result<Analysis, String> {
    let request = ureq::get(&format!("{base}/api/books/structure"))
        .query("include_paragraphs", "true")
        .query("detail", detail)
        .query("gutenberg_id", &gutenberg_id.to_string())
        .timeout(TIMEOUT);

    match request.call() {
        Ok(resp) => parse_analysis(resp),
        Err(ureq::Error::Status(code, resp)) => Err(status_message(code, resp)),
        Err(err) => Err(unreachable_message(base, &err)),
    }
}

/// POST /api/books/upload — upload an EPUB file for analysis.
pub fn upload(base: &str, path: &Path, detail: &str) -> Result<Analysis, String> {
    let bytes = std::fs::read(path)
        .map_err(|err| format!("Couldn't read “{}”: {err}", path.display()))?;
    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "book.epub".into());

    let (body, content_type) = multipart_body(&filename, &bytes);

    let result = ureq::post(&format!("{base}/api/books/upload"))
        .query("include_paragraphs", "true")
        .query("detail", detail)
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
    let text = read_body(resp)?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|err| format!("The API returned invalid JSON: {err}"))?;

    let title = value["book"]["title"]
        .as_str()
        .unwrap_or("Untitled")
        .to_string();
    let tree = build_tree(&value["structure"]);
    let (schema_json, ranges) = pretty_print_with_ranges(&value);

    Ok(Analysis { title, schema_json, tree, ranges })
}

// ── Pretty printer with node line ranges ───────────────────────────────────────
//
// Serialises the response like `to_string_pretty` (2-space indent) while
// recording which lines each structure node occupies, keyed by the same
// positional ids the tree uses ("n0", "n0.2", "n0.2.4", …).

/// Where the writer currently is, for id assignment.
#[derive(Clone)]
enum NodeCtx {
    Outside,
    /// Inside the top-level "structure" object.
    Structure,
    /// Inside a structure node object with this id.
    Node(String),
}

fn pretty_print_with_ranges(value: &Value) -> (String, HashMap<String, (usize, usize)>) {
    let mut out = String::new();
    let mut line = 0usize;
    let mut ranges = HashMap::new();
    write_value(value, 0, &NodeCtx::Outside, &mut out, &mut line, &mut ranges);
    (out, ranges)
}

fn push(out: &mut String, line: &mut usize, s: &str) {
    *line += s.matches('\n').count();
    out.push_str(s);
}

fn write_value(
    value: &Value,
    indent: usize,
    ctx: &NodeCtx,
    out: &mut String,
    line: &mut usize,
    ranges: &mut HashMap<String, (usize, usize)>,
) {
    let pad = " ".repeat(indent + 2);
    match value {
        Value::Object(map) if !map.is_empty() => {
            push(out, line, "{\n");
            for (ix, (key, val)) in map.iter().enumerate() {
                push(out, line, &format!("{pad}{}: ", Value::String(key.clone())));
                let child_ctx = match (ctx, key.as_str()) {
                    (NodeCtx::Outside, "structure") if indent == 0 => NodeCtx::Structure,
                    (NodeCtx::Structure, "nodes") => NodeCtx::Structure,
                    (
                        NodeCtx::Node(_),
                        "children" | "paragraphs" | "sentences" | "clauses" | "words",
                    ) => ctx.clone(),
                    _ => NodeCtx::Outside,
                };
                write_value(val, indent + 2, &child_ctx, out, line, ranges);
                let sep = if ix + 1 < map.len() { ",\n" } else { "\n" };
                push(out, line, sep);
            }
            push(out, line, &format!("{}}}", " ".repeat(indent)));
        }
        Value::Array(items) if !items.is_empty() => {
            push(out, line, "[\n");
            for (ix, item) in items.iter().enumerate() {
                push(out, line, &pad);
                let start = *line;
                let item_ctx = element_ctx(ctx, ix);
                write_value(item, indent + 2, &item_ctx, out, line, ranges);
                if let NodeCtx::Node(id) = &item_ctx {
                    ranges.entry(id.clone()).or_insert((start, *line));
                }
                let sep = if ix + 1 < items.len() { ",\n" } else { "\n" };
                push(out, line, sep);
            }
            push(out, line, &format!("{}]", " ".repeat(indent)));
        }
        other => push(out, line, &other.to_string()),
    }
}

/// The context for element `ix` of an array written under `ctx`.
fn element_ctx(ctx: &NodeCtx, ix: usize) -> NodeCtx {
    match ctx {
        NodeCtx::Structure => NodeCtx::Node(format!("n{ix}")),
        NodeCtx::Node(parent) => NodeCtx::Node(format!("{parent}.{ix}")),
        NodeCtx::Outside => NodeCtx::Outside,
    }
}

// ── Structure → label tree ─────────────────────────────────────────────────────
//
// Node shapes from the API (models/structure.py):
//   PartNode      {level, index, label, children: [ChapterNode]}
//   ChapterNode   {level, index, label, paragraphs: [ParagraphNode] | null}
//   ParagraphNode {index, sentence_count, word_count}
// canonical_scripture leaves are verses; every other schema's leaves are
// paragraphs. Labels never contain book text.

fn build_tree(structure: &Value) -> Vec<TreeNode> {
    let leaf_name = if structure["schema"].as_str() == Some("canonical_scripture") {
        "Verse"
    } else {
        "Paragraph"
    };

    let Some(nodes) = structure["nodes"].as_array() else {
        return Vec::new();
    };
    nodes
        .iter()
        .enumerate()
        .map(|(ix, node)| top_node(node, ix, leaf_name))
        .collect()
}

fn top_node(node: &Value, ix: usize, leaf_name: &str) -> TreeNode {
    // Ids are positional and must mirror pretty_print_with_ranges exactly.
    let id = format!("n{ix}");
    if let Some(children) = node["children"].as_array() {
        // Part / volume / section / book
        TreeNode {
            label: node_label(node, "Section"),
            children: children
                .iter()
                .enumerate()
                .map(|(cx_ix, c)| chapter_node(c, format!("{id}.{cx_ix}"), leaf_name))
                .collect(),
            id,
        }
    } else if node["paragraph_count"].is_number() {
        chapter_node(node, id, leaf_name)
    } else {
        paragraph_node(node, id, leaf_name)
    }
}

fn chapter_node(node: &Value, id: String, leaf_name: &str) -> TreeNode {
    let children = child_nodes(node, "paragraphs", &id, |p, cid| {
        paragraph_node(p, cid, leaf_name)
    });
    TreeNode {
        label: node_label(node, "Chapter"),
        children,
        id,
    }
}

/// Map the elements of `node[key]` (if present) through `build`, extending ids
/// positionally — the same scheme the pretty printer records ranges under.
fn child_nodes(
    node: &Value,
    key: &str,
    parent_id: &str,
    build: impl Fn(&Value, String) -> TreeNode,
) -> Vec<TreeNode> {
    node[key]
        .as_array()
        .map(|items| {
            items
                .iter()
                .enumerate()
                .map(|(ix, item)| build(item, format!("{parent_id}.{ix}")))
                .collect()
        })
        .unwrap_or_default()
}

fn paragraph_node(node: &Value, id: String, leaf_name: &str) -> TreeNode {
    let index = node["index"].as_u64().unwrap_or(0);
    let sentences = node["sentence_count"].as_u64().unwrap_or(0);
    let words = node["word_count"].as_u64().unwrap_or(0);
    TreeNode {
        children: child_nodes(node, "sentences", &id, sentence_node),
        id,
        label: format!("{leaf_name} {index} — {sentences} sentences · {words} words"),
    }
}

fn sentence_node(node: &Value, id: String) -> TreeNode {
    let index = node["index"].as_u64().unwrap_or(0);
    let clauses = node["clause_count"].as_u64().unwrap_or(0);
    let words = node["word_count"].as_u64().unwrap_or(0);
    TreeNode {
        children: child_nodes(node, "clauses", &id, clause_node),
        id,
        label: format!("Sentence {index} — {clauses} clauses · {words} words"),
    }
}

fn clause_node(node: &Value, id: String) -> TreeNode {
    let index = node["index"].as_u64().unwrap_or(0);
    let words = node["word_count"].as_u64().unwrap_or(0);
    TreeNode {
        children: child_nodes(node, "words", &id, word_node),
        id,
        label: format!("Clause {index} — {words} words"),
    }
}

fn word_node(node: &Value, id: String) -> TreeNode {
    let index = node["index"].as_u64().unwrap_or(0);
    TreeNode {
        id,
        label: format!("Word {index}"),
        children: Vec::new(),
    }
}

fn node_label(node: &Value, fallback_kind: &str) -> String {
    if let Some(label) = node["label"].as_str() {
        if !label.trim().is_empty() {
            return label.trim().to_string();
        }
    }
    let kind = node["level"].as_str().unwrap_or(fallback_kind);
    format!("{kind} {}", node["index"].as_u64().unwrap_or(0))
}

fn parse_matches(resp: ureq::Response) -> Result<Vec<BookMatch>, String> {
    let value: Value = read_body(resp).and_then(|text| {
        serde_json::from_str(&text).map_err(|err| format!("Invalid JSON from the API: {err}"))
    })?;

    let matches = value["matches"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    Some(BookMatch {
                        gutenberg_id: entry["gutenberg_id"].as_u64()?,
                        title: entry["title"].as_str().unwrap_or("Untitled").to_string(),
                        authors: entry["authors"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(Value::as_str)
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .unwrap_or_default(),
                        language: entry["language"].as_str().unwrap_or("").to_string(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if matches.is_empty() {
        return Err("Multiple books matched but the list couldn't be read. Try an ISBN.".into());
    }
    Ok(matches)
}

fn status_message(code: u16, resp: ureq::Response) -> String {
    let detail = read_body(resp)
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
