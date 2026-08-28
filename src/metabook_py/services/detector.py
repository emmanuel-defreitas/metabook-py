"""
Schema detection service.

Detection pipeline (priority order — first match wins):

  1. canonical_scripture  — verse number patterns (1:1 …) dominate
  2. sectioned_book       — PART / VOLUME / SECTION markers + chapters
  3. standard_book        — CHAPTER markers or standalone Roman/Arabic numerals
  4. essay_collection     — ALL-CAPS standalone titles, no chapter markers
  5. flat                 — fallback; pure paragraph stream

Confidence:
  high   ≥ 5 structural markers found
  medium   2–4 markers
  low      1 marker
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from enum import StrEnum

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


# ── Compiled regex patterns ────────────────────────────────────────────────────

# Scripture: verse numbers at the start of a line, e.g. "1:1 In the beginning"
VERSE_RE = re.compile(r"^\d+:\d+\s", re.MULTILINE)

# Scripture: recognised book names as standalone lines
SCRIPTURE_BOOK_RE = re.compile(
    r"^(?:Genesis|Exodus|Leviticus|Numbers|Deuteronomy|Joshua|Judges|Ruth|"
    r"(?:First|Second|Third|Fourth)?\s*(?:Samuel|Kings|Chronicles)|Ezra|"
    r"Nehemiah|Esther|Job|Psalms?|Proverbs|Ecclesiastes|Song of Solomon|"
    r"Isaiah|Jeremiah|Lamentations|Ezekiel|Daniel|Hosea|Joel|Amos|Obadiah|"
    r"Jonah|Micah|Nahum|Habakkuk|Zephaniah|Haggai|Zechariah|Malachi|"
    r"Matthew|Mark|Luke|John|Acts|Romans|"
    r"(?:First|Second)?\s*Corinthians|Galatians|Ephesians|Philippians|"
    r"Colossians|(?:First|Second)?\s*Thessalonians|"
    r"(?:First|Second)?\s*Timothy|Titus|Philemon|Hebrews|James|"
    r"(?:First|Second|Third)?\s*(?:Peter|John)|Jude|Revelation|"
    r"BOOK\s+(?:OF\s+)?[A-Z][A-Z\s]+)\s*$",
    re.IGNORECASE | re.MULTILINE,
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
CAPS_TITLE_RE = re.compile(r"^[A-Z][A-Z\s''\"—\-]{3,60}$", re.MULTILINE)


# ── Helpers ────────────────────────────────────────────────────────────────────


def _confidence(count: int) -> str:
    if count >= 5:
        return "high"
    if count >= 2:
        return "medium"
    return "low"


def _findall(pattern: re.Pattern, text: str) -> list[str]:
    return pattern.findall(text)


# ── Public API ─────────────────────────────────────────────────────────────────


def detect_schema(text: str) -> DetectedSchema:
    """
    Analyse raw (boilerplate-stripped) text and return the best-matching
    structural schema together with a confidence score.
    """

    # 1 ── Canonical scripture (verse-number pattern is a very strong signal)
    verses = _findall(VERSE_RE, text)
    if len(verses) >= 10:
        return DetectedSchema(
            name=SchemaType.CANONICAL_SCRIPTURE,
            confidence=_confidence(len(verses)),
            markers_found=len(verses),
        )

    # 2 ── Sectioned book (parts AND chapters)
    parts = _findall(PART_RE, text)
    chapters_word = _findall(CHAPTER_WORD_RE, text)
    if len(parts) >= 2 and len(chapters_word) >= 2:
        total = len(parts) + len(chapters_word)
        return DetectedSchema(
            name=SchemaType.SECTIONED_BOOK,
            confidence=_confidence(total),
            markers_found=total,
        )

    # 3 ── Standard book (chapters only)
    chapters_num = _findall(CHAPTER_NUM_RE, text)
    best_chapters = chapters_word if len(chapters_word) >= len(chapters_num) else chapters_num
    if len(best_chapters) >= 2:
        return DetectedSchema(
            name=SchemaType.STANDARD_BOOK,
            confidence=_confidence(len(best_chapters)),
            markers_found=len(best_chapters),
        )

    # 4 ── Essay / story collection (ALL-CAPS titles, no chapter markers)
    caps_titles = [t for t in _findall(CAPS_TITLE_RE, text) if not _STRUCTURAL_KEYWORDS.search(t)]
    if len(caps_titles) >= 2:
        return DetectedSchema(
            name=SchemaType.ESSAY_COLLECTION,
            confidence=_confidence(len(caps_titles)),
            markers_found=len(caps_titles),
        )

    # 5 ── Flat fallback
    paragraph_count = len([p for p in text.split("\n\n") if p.strip()])
    return DetectedSchema(
        name=SchemaType.FLAT,
        confidence="high",
        markers_found=paragraph_count,
    )
