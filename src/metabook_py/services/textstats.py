"""
Shared text statistics: sentence splitting and word counting.

Both the book counter and the single-document analyzer use these functions,
so a sentence or a word means the same thing everywhere in the API.

Rules
-----
- Sentence : split on [.!?] followed by whitespace + an uppercase letter or
             opening quote, with common abbreviations ("Mr.", "etc.") protected.
- Word     : any token matching \\b[a-zA-Z0-9]+\\b.

With METABOOK_PARSER=rust these delegate to metabook_core (identical output,
enforced by tests/test_parity.py).
"""

from __future__ import annotations

import re

from metabook_py.services.native import native_core

ABBREVIATIONS = frozenset(
    w.lower()
    for w in (
        "Mr Mrs Ms Dr Prof Sr Jr Rev Gen Col Maj Sgt Cpl Lt Cmdr Adm "
        "Gov Sen Rep St Ave Blvd Dept est approx vol no pp fig "
        "Jan Feb Mar Apr Jun Jul Aug Sep Oct Nov Dec "
        "Inc Ltd Corp Bros Co vs etc al ibid op cit"
    ).split()
)

# A 2–6 letter word ending in "." — the abbreviation check is a set lookup in
# the callback. One cheap pattern plus a lookup is ~2x faster than a 50-way
# case-insensitive alternation, and matches exactly the same spans.
_ABBREV_CANDIDATE = re.compile(r"\b([A-Za-z]{2,6})\.")
_PLACEHOLDER = "<!P!>"

# Python's re module requires fixed-width look-behinds, so we match on the
# punctuation character itself (no trailing quote in look-behind).
SENTENCE_BOUNDARY = re.compile(r"(?<=[.!?])\s+(?=[A-Z\"‘“])")
_WORD = re.compile(r"\b[a-zA-Z0-9]+\b")


def _protect(match: re.Match[str]) -> str:
    word = match.group(1)
    return word + _PLACEHOLDER if word.lower() in ABBREVIATIONS else match.group(0)


def split_sentences(text: str) -> list[str]:
    """Split a block of prose into sentences, protecting abbreviations."""
    if (core := native_core()) is not None:
        return core.split_sentences(text)
    return _split_sentences_py(text)


def _split_sentences_py(text: str) -> list[str]:
    protected = _ABBREV_CANDIDATE.sub(_protect, text)
    parts = SENTENCE_BOUNDARY.split(protected)
    return [s.replace(_PLACEHOLDER, ".").strip() for s in parts if s.strip()]


def count_sentences(text: str) -> int:
    """Sentence count of a non-empty prose block (at least 1), 0 for blank text."""
    if (core := native_core()) is not None:
        return core.count_sentences(text)
    if not text.strip():
        return 0
    return max(1, len(_split_sentences_py(text)))


def count_words(text: str) -> int:
    if (core := native_core()) is not None:
        return core.count_words(text)
    return len(_WORD.findall(text))
