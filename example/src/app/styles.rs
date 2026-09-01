//! Shared presentation and form option constants.

/// Fixed widths keep adjacent form controls stable while the query field grows.
pub(super) const ISBN_FIELD_WIDTH: f32 = 176.;
pub(super) const DETAIL_FIELD_WIDTH: f32 = 144.;

/// Search route tab index.
pub(super) const TAB_SEARCH: usize = 0;
/// Upload route tab index.
pub(super) const TAB_UPLOAD: usize = 1;

/// Options for the detail select, index-aligned with `DETAIL_VALUES`.
pub(super) const DETAIL_OPTIONS: [&str; 4] = ["Paragraphs", "Sentences", "Clauses", "Words"];
pub(super) const DETAIL_VALUES: [&str; 4] = ["paragraph", "sentence", "clause", "word"];

/// Tokenizer labels sent to the API verbatim, except for "No tokens".
pub(super) const TOKENIZER_OPTIONS: [&str; 6] = [
    "No tokens",
    "bert-base-uncased",
    "gpt2",
    "roberta-base",
    "distilbert-base-uncased",
    "xlm-roberta-base",
];

/// Default tokenizer: `bert-base-uncased`.
pub(super) const TOKENIZER_DEFAULT_IX: usize = 1;
