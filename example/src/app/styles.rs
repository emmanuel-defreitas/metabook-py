//! Shared presentation and form option constants.

/// Detail choices for structural scans.
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
