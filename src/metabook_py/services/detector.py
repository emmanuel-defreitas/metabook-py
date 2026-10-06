"""
Schema detection service.

Detection pipeline (priority order — first match wins):

  1. canonical_scripture  — verse number patterns (1:1 …) dominate
  2. sectioned_book       — PART / VOLUME / SECTION markers + chapters
  3. standard_book        — CHAPTER markers or standalone Roman/Arabic numerals
  4. essay_collection     — ALL-CAPS standalone titles, no chapter markers
  5. flat                 — fallback; pure paragraph stream

Confidence:
  high     score ≥ 0.8 (normally ≥ 5 markers)
  medium   score ≥ 0.5
  low      weak / missing evidence; flat is a fallback, not certainty

All-caps titles without repeated bylines are capped at medium; competing
schema families cap the selected score at 0.6.

Numeric scores expose heuristic rule support, not calibrated probabilities.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from enum import StrEnum

from metabook_py.services.preprocessing import preprocess_text

# ── Schema taxonomy ────────────────────────────────────────────────────────────


class SchemaType(StrEnum):
    CANONICAL_SCRIPTURE = "canonical_scripture"
    SECTIONED_BOOK = "sectioned_book"
    STANDARD_BOOK = "standard_book"
    ESSAY_COLLECTION = "essay_or_story_collection"
    FLAT = "flat"


SCHEMA_DEFINITIONS: dict[SchemaType, dict] = {
    SchemaType.CANONICAL_SCRIPTURE: {
        "name": "canonical_scripture",
        "description": "Scripture / Canon — Book → Chapter → Verse",
        "hierarchy": ["book", "chapter", "verse"],
    },
    SchemaType.SECTIONED_BOOK: {
        "name": "sectioned_book",
        "description": "Sectioned Book — Part/Volume/Section → Chapter → Paragraph",
        "hierarchy": ["part", "chapter", "paragraph"],
    },
    SchemaType.STANDARD_BOOK: {
        "name": "standard_book",
        "description": "Standard Book — Chapter → Paragraph",
        "hierarchy": ["chapter", "paragraph"],
    },
    SchemaType.ESSAY_COLLECTION: {
        "name": "essay_or_story_collection",
        "description": "Essay / Story Collection — Essay → Paragraph",
        "hierarchy": ["essay", "paragraph"],
    },
    SchemaType.FLAT: {
        "name": "flat",
        "description": "Flat — Paragraph stream only",
        "hierarchy": ["paragraph"],
    },
}


@dataclass
class DetectedSchema:
    name: SchemaType
    confidence: str  # "high" | "medium" | "low"
    markers_found: int
    score: float = 0.0
    evidence: dict[str, int] = field(default_factory=dict)
    candidate_scores: dict[str, float] = field(default_factory=dict)
    detected_name: SchemaType | None = None
    overridden: bool = False

    def explanation(self) -> dict:
        """Additive fields shared by HTTP and MCP, containing no source spans."""
        return {
            "schema_score": self.score,
            "schema_evidence": self.evidence,
            "schema_candidates": self.candidate_scores,
            "schema_detected": (self.detected_name or self.name).value,
            "schema_overridden": self.overridden,
        }


# ── Compiled regex patterns ────────────────────────────────────────────────────

# Scripture: verse numbers at the start of a line, e.g. "1:1 In the beginning"
VERSE_RE = re.compile(r"^\d+:\d+\s", re.MULTILINE)

# Canonical scripture book names, shared by the two heading forms below.
_SCRIPTURE_BOOK_NAMES = (
    r"Genesis|Exodus|Leviticus|Numbers|Deuteronomy|Joshua|Judges|Ruth|"
    r"(?:First|Second|Third|Fourth)?\s*(?:Samuel|Kings|Chronicles)|Ezra|"
    r"Nehemiah|Esther|Job|Psalms?|Proverbs|Ecclesiastes|Song of Solomon|"
    r"Isaiah|Jeremiah|Lamentations|Ezekiel|Daniel|Hosea|Joel|Amos|Obadiah|"
    r"Jonah|Micah|Nahum|Habakkuk|Zephaniah|Haggai|Zechariah|Malachi|"
    r"Matthew|Mark|Luke|John|Acts|Romans|"
    r"(?:First|Second)?\s*Corinthians|Galatians|Ephesians|Philippians|"
    r"Colossians|(?:First|Second)?\s*Thessalonians|"
    r"(?:First|Second)?\s*Timothy|Titus|Philemon|Hebrews|James|"
    r"(?:First|Second|Third)?\s*(?:Peter|John)|Jude|Revelation"
)

# Scripture: book headings as their own paragraph (blank-line separated), in
# two forms: a bare book name ("GENESIS"), or a descriptive title containing a
# book name ("The First Book of Moses: Called Genesis"). Anchoring both ends
# to a paragraph boundary excludes wrapped verse lines and tables of contents
# (whose entries sit on consecutive lines).
SCRIPTURE_BOOK_RE = re.compile(
    rf"(?:\A|(?<=\n\n))"
    rf"(?:(?:{_SCRIPTURE_BOOK_NAMES})|"
    rf"BOOK\s+(?:OF\s+)?[A-Z][A-Z\s]+|"
    rf"The\s(?=[^\n]*\b(?:{_SCRIPTURE_BOOK_NAMES})\b)[^\n]{{0,69}}[^\s.!?])"
    rf"[ \t]*(?=\n\n|\n?\Z)",
    re.IGNORECASE,
)

# Parts / Volumes / Sections — top-level structural markers
PART_RE = re.compile(
    r"^(?:PART|VOLUME|SECTION|BOOK)\s+"
    r"(?:[IVXLC]{1,8}|[0-9]{1,3}|"
    r"ONE|TWO|THREE|FOUR|FIVE|SIX|SEVEN|EIGHT|NINE|TEN|"
    r"FIRST|SECOND|THIRD|FOURTH|FIFTH|SIXTH|SEVENTH|EIGHTH|NINTH|TENTH)"
    r"(?:[ \t]*[:—\-]?[ \t]*.{0,60})?$",  # [ \t]* not \s* — must not eat \n
    re.IGNORECASE | re.MULTILINE,
)

# Chapters — explicit word "Chapter" (or abbreviations)
CHAPTER_WORD_RE = re.compile(
    r"^(?:CHAPTER|CHAP\.?|CH\.?)\s+"
    r"(?:[IVXLC]{1,8}|[0-9]{1,3}|"
    r"ONE|TWO|THREE|FOUR|FIVE|SIX|SEVEN|EIGHT|NINE|TEN|"
    r"ELEVEN|TWELVE|THIRTEEN|FOURTEEN|FIFTEEN|"
    r"TWENTIETH|THIRTIETH|FORTIETH|FIFTIETH|"
    r"FIRST|SECOND|THIRD|FOURTH|FIFTH|SIXTH|SEVENTH|EIGHTH|NINTH|TENTH)"
    r"(?:[ \t]*[:—\-]?[ \t]*.{0,80})?$",  # [ \t]* not \s* — must not eat \n
    re.IGNORECASE | re.MULTILINE,
)

# Chapters — standalone Roman or Arabic numeral on its own line (e.g. "IV." or "12.")
CHAPTER_NUM_RE = re.compile(
    r"^(?:[IVXLC]{1,6}|[0-9]{1,3})\.\s*$",
    re.MULTILINE,
)

# ALL-CAPS essay / story titles — standalone line, not a structural keyword
_STRUCTURAL_KEYWORDS = re.compile(
    r"\b(?:CHAPTER|PART|VOLUME|SECTION|PREFACE|INTRODUCTION|APPENDIX|"
    r"CONTENTS|INDEX|EPILOGUE|PROLOGUE|FOREWORD|AFTERWORD|BOOK)\b",
    re.IGNORECASE,
)
CAPS_TITLE_RE = re.compile(r"^[A-Z][A-Z \t''\"—\-]{3,60}$", re.MULTILINE)


# ── Helpers ────────────────────────────────────────────────────────────────────


def _findall(pattern: re.Pattern, text: str) -> list[str]:
    return pattern.findall(text)


# ── Public API ─────────────────────────────────────────────────────────────────


def detect_schema(text: str, *, schema_override: SchemaType | None = None) -> DetectedSchema:
    """Select a schema with an explainable heuristic score, not a probability.

    Candidate scores represent rule support, not mutually exclusive likelihoods.
    An override changes the builder selection, never the automatic evidence.
    """
    text = preprocess_text(text)
    verses = len(_findall(VERSE_RE, text))
    parts = len(_findall(PART_RE, text))
    chapters_word = len(_findall(CHAPTER_WORD_RE, text))
    chapters_num = len(_findall(CHAPTER_NUM_RE, text))
    chapters = max(chapters_word, chapters_num)
    titles = [m for m in CAPS_TITLE_RE.finditer(text) if not _STRUCTURAL_KEYWORDS.search(m[0])]
    # Repeated title + explicit "By ..." lines strengthen the otherwise weak
    # all-caps heuristic. Return only a count, never an author or title excerpt.
    bylines = sum(
        bool(
            re.match(r"[ \t]*\n+(?:[ \t]*\n)*[ \t]*By[ \t]+[^\n]+", text[m.end() :], re.IGNORECASE)
        )
        for m in titles
    )
    paragraph_count = len([p for p in text.split("\n\n") if p.strip()])
    evidence = {
        "verse_number_lines": verses,
        "part_markers": parts,
        "chapter_word_markers": chapters_word,
        "chapter_numeral_markers": chapters_num,
        "caps_title_lines": len(titles),
        "title_byline_pairs": bylines,
        "paragraph_blocks": paragraph_count,
    }

    def strength(count: int) -> float:
        return round(min(0.9, 0.3 + 0.1 * count), 2) if count else 0.0

    scores = {
        SchemaType.CANONICAL_SCRIPTURE.value: strength(verses)
        if verses >= 10
        else round(0.03 * verses, 2),
        SchemaType.SECTIONED_BOOK.value: strength(parts + chapters_word)
        if parts >= 2 and chapters_word >= 2
        else 0.0,
        SchemaType.STANDARD_BOOK.value: strength(chapters),
        SchemaType.ESSAY_COLLECTION.value: min(
            strength(len(titles)), 0.85 if bylines >= 2 else 0.65
        ),
        SchemaType.FLAT.value: 0.2 if not (verses or parts or chapters or titles) else 0.1,
    }
    if verses >= 10:
        name, count = SchemaType.CANONICAL_SCRIPTURE, verses
    elif parts >= 2 and chapters_word >= 2:
        name, count = SchemaType.SECTIONED_BOOK, parts + chapters_word
    elif chapters >= 2:
        name, count = SchemaType.STANDARD_BOOK, chapters
    elif len(titles) >= 2:
        name, count = SchemaType.ESSAY_COLLECTION, len(titles)
    else:
        name, count = SchemaType.FLAT, paragraph_count

    # Keep the established rule priority, but expose competing support and
    # reduce confidence when a different schema family is also plausible.
    competing = (int(verses >= 10) + int(chapters >= 2) + int(len(titles) >= 2)) > 1
    score = min(scores[name.value], 0.6) if competing else scores[name.value]
    confidence = "high" if score >= 0.8 else "medium" if score >= 0.5 else "low"
    return DetectedSchema(
        name=SchemaType(schema_override) if schema_override is not None else name,
        confidence=confidence,
        markers_found=count,
        score=score,
        evidence=evidence,
        candidate_scores=scores,
        detected_name=name,
        overridden=schema_override is not None,
    )
