//! Sentence splitting and word counting — port of services/textstats.py.

use std::borrow::Cow;

use crate::pystr;

const ABBREVIATIONS: &[&str] = &[
    "mr", "mrs", "ms", "dr", "prof", "sr", "jr", "rev", "gen", "col", "maj", "sgt", "cpl", "lt",
    "cmdr", "adm", "gov", "sen", "rep", "st", "ave", "blvd", "dept", "est", "approx", "vol", "no",
    "pp", "fig", "jan", "feb", "mar", "apr", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    "inc", "ltd", "corp", "bros", "co", "vs", "etc", "al", "ibid", "op", "cit",
];
const PLACEHOLDER: &str = "<!P!>";

/// Python: `re.sub(r"\b([A-Za-z]{2,6})\.", …)` marking abbreviations. A
/// candidate is a maximal ASCII-letter run of 2–6 starting at a Python word
/// boundary and followed by "." (backtracking can't produce other matches).
fn protect(text: &str) -> Cow<'_, str> {
    let bytes = text.as_bytes();
    let mut out: Option<String> = None;
    let mut copied = 0;
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_alphabetic() || !pystr::word_start(text, i) {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < bytes.len() && bytes[j].is_ascii_alphabetic() {
            j += 1;
        }
        let run = j - i;
        if (2..=6).contains(&run)
            && bytes.get(j) == Some(&b'.')
            && ABBREVIATIONS.contains(&text[i..j].to_ascii_lowercase().as_str())
        {
            let o = out.get_or_insert_with(|| String::with_capacity(text.len() + 16));
            o.push_str(&text[copied..j]);
            o.push_str(PLACEHOLDER);
            copied = j + 1;
            i = j + 1;
        } else {
            // A non-abbreviation candidate is consumed whole (re.sub moves on
            // past it); either way the next possible start is after the run.
            i = j;
        }
    }
    match out {
        Some(mut o) => {
            o.push_str(&text[copied..]);
            Cow::Owned(o)
        }
        None => Cow::Borrowed(text),
    }
}

/// Python: `(?<=[.!?])\s+(?=[A-Z"‘“])` split. A boundary is the
/// whitespace run right after `.`, `!` or `?` when the next character starts
/// a sentence.
fn split_boundaries(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if !matches!(c, '.' | '!' | '?') {
            continue;
        }
        let ws_start = i + c.len_utf8();
        let mut ws_end = ws_start;
        while let Some(&(j, w)) = chars.peek() {
            if !pystr::is_space(w) {
                break;
            }
            ws_end = j + w.len_utf8();
            chars.next();
        }
        if ws_end == ws_start {
            continue;
        }
        if let Some(&(_, next)) = chars.peek()
            && (next.is_ascii_uppercase() || matches!(next, '"' | '\u{2018}' | '\u{201C}'))
        {
            parts.push(&text[start..ws_start]);
            start = ws_end;
        }
    }
    parts.push(&text[start..]);
    parts
}

pub fn split_sentences(text: &str) -> Vec<String> {
    let protected = protect(text);
    split_boundaries(&protected)
        .into_iter()
        .filter(|s| !pystr::strip(s).is_empty())
        .map(|s| pystr::strip(&s.replace(PLACEHOLDER, ".")).to_string())
        .collect()
}

pub fn count_sentences(text: &str) -> usize {
    if pystr::strip(text).is_empty() {
        return 0;
    }
    let protected = protect(text);
    let n = split_boundaries(&protected)
        .into_iter()
        .filter(|s| !pystr::strip(s).is_empty())
        .count();
    n.max(1)
}

/// Python: `len(re.findall(r"\b[a-zA-Z0-9]+\b", text))` — a maximal ASCII
/// alphanumeric run counts when neither neighbour is a Python word char.
pub fn count_words(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut n = 0;
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_alphanumeric() {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < bytes.len() && bytes[j].is_ascii_alphanumeric() {
            j += 1;
        }
        let ends = text[j..].chars().next().is_none_or(|c| !pystr::is_word(c));
        if ends && pystr::word_start(text, i) {
            n += 1;
        }
        i = j;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abbreviations_do_not_split() {
        let s = split_sentences("Mr. Brown met Dr. Who. They talked.");
        assert_eq!(s, vec!["Mr. Brown met Dr. Who.", "They talked."]);
    }

    #[test]
    fn boundary_needs_uppercase_or_quote() {
        assert_eq!(split_sentences("one. two. Three").len(), 2);
        assert_eq!(split_sentences("Hi! \"Yes,\" she said.").len(), 2);
    }

    #[test]
    fn words() {
        assert_eq!(count_words("a well-known fact"), 4);
        assert_eq!(count_words("Chapter 1 has 5 words total"), 6);
        assert_eq!(count_words("café"), 0); // \b never falls between f and é
        assert_eq!(count_words("x² y"), 1); // ² is a Python word char
    }

    #[test]
    fn empty() {
        assert!(split_sentences("").is_empty());
        assert_eq!(count_sentences("   "), 0);
        assert_eq!(count_sentences("no terminal punctuation"), 1);
    }
}
