//! Python `str` semantics the ports rely on.
//!
//! Python's `str.isspace()` / regex `\s` include U+001C..U+001F (bidi
//! separators), which Rust's `char::is_whitespace` does not. Everything here
//! matches the Python definition so outputs stay identical.

/// `str.isspace()` for one character.
pub fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

// In regexes, Python's `\s` is spelled `[\s\x1c-\x1f]` for the same reason.

static PY_WORD: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^[\p{L}\p{N}_]$").unwrap());
static PY_ALPHA: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^\p{L}$").unwrap());

/// Python regex `\w` (str patterns): `c.isalnum() or c == "_"`. Unlike Rust's
/// `\w` it includes all numerics (e.g. `²`) and excludes combining marks.
pub fn is_word(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_alphanumeric() || c == '_';
    }
    let mut buf = [0u8; 4];
    PY_WORD.is_match(c.encode_utf8(&mut buf))
}

/// `str.isalpha()` for one character (Unicode category L*).
pub fn is_alpha(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_alphabetic();
    }
    let mut buf = [0u8; 4];
    PY_ALPHA.is_match(c.encode_utf8(&mut buf))
}

/// Python `\b` before position `i` when the next character is a word char:
/// true when the previous character is not a word char.
pub fn word_start(text: &str, i: usize) -> bool {
    text[..i].chars().next_back().is_none_or(|p| !is_word(p))
}

/// `str.strip()`.
pub fn strip(s: &str) -> &str {
    s.trim_matches(is_space)
}

/// `str.rstrip()`.
pub fn rstrip(s: &str) -> &str {
    s.trim_end_matches(is_space)
}

/// `str.split()` with no arguments.
pub fn split_ws(s: &str) -> impl Iterator<Item = &str> {
    s.split(is_space).filter(|p| !p.is_empty())
}

/// `re.sub(r"\s+", " ", s).strip()`.
pub fn collapse_ws(s: &str) -> String {
    split_ws(s).collect::<Vec<_>>().join(" ")
}

/// `len(s)` (code points).
pub fn len(s: &str) -> usize {
    s.chars().count()
}

/// Python's text normalization for line splitting: CRLF and CR → LF.
pub fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}
