"""
Response models for the structural analysis.

Node hierarchy:
  canonical_scripture  →  PartNode(book)  →  ChapterNode(chapter)  →  ParagraphNode(verse)
  sectioned_book       →  PartNode(part)  →  ChapterNode(chapter)  →  ParagraphNode
  standard_book        →  ChapterNode(chapter)  →  ParagraphNode
  essay_collection     →  ChapterNode(essay)   →  ParagraphNode
  flat                 →  ParagraphNode
"""

from __future__ import annotations

from datetime import datetime
from typing import Any

from pydantic import BaseModel, Field

from metabook_py.models.book import BlobInfo, BookInfo, UploadedBookInfo

# ── Deep-detail nodes (opt-in via detail=sentence|clause|word) ─────────────────
#
# Like every other node these carry only positions and counts — never text.
# A word node is purely positional.


class WordNode(BaseModel):
    index: int


class ClauseNode(BaseModel):
    index: int
    word_count: int
    words: list[WordNode] | None = None  # None unless detail="word"


class SentenceNode(BaseModel):
    index: int
    clause_count: int
    word_count: int
    clauses: list[ClauseNode] | None = None  # None unless detail>="clause"


# ── Leaf node ──────────────────────────────────────────────────────────────────


class ParagraphNode(BaseModel):
    index: int
    sentence_count: int
    word_count: int
    avg_words_per_sentence: float
    sentences: list[SentenceNode] | None = None  # None unless detail>="sentence"


# ── Mid-level node (chapter / essay / story) ───────────────────────────────────


class ChapterNode(BaseModel):
    level: str  # "chapter" | "essay" | "story"
    index: int
    label: str
    paragraph_count: int
    avg_sentences_per_paragraph: float
    avg_words_per_sentence: float
    total_words: int
    total_sentences: int
    paragraphs: list[ParagraphNode] | None = None  # None when include_paragraphs=False


# ── Top-level node (part / volume / section / book) ────────────────────────────


class PartNode(BaseModel):
    level: str  # "part" | "volume" | "section" | "book"
    index: int
    label: str
    child_count: int
    total_paragraphs: int
    total_words: int
    children: list[ChapterNode]


# ── Aggregates ─────────────────────────────────────────────────────────────────


class StructureSummary(BaseModel):
    total_top_level_nodes: int
    total_mid_level_nodes: int | None = None  # None for flat / standard_book
    total_paragraphs: int
    total_sentences: int
    total_words: int
    avg_paragraphs_per_chapter: float
    avg_sentences_per_paragraph: float
    avg_words_per_sentence: float


# ── Top-level response ─────────────────────────────────────────────────────────


class StructureDetail(BaseModel):
    schema_type: str = Field(alias="schema")
    schema_confidence: str  # "high" | "medium" | "low"
    summary: StructureSummary
    nodes: list[Any]  # list[PartNode | ChapterNode | ParagraphNode]

    model_config = {"populate_by_name": True}


class MetaInfo(BaseModel):
    fetched_at: datetime
    cached: bool
    processing_time_ms: int


class BookStructureResponse(BaseModel):
    book: BookInfo
    structure: StructureDetail
    meta: MetaInfo


# ── Upload response ────────────────────────────────────────────────────────────


class UploadMetaInfo(BaseModel):
    uploaded_at: datetime
    spine_document_count: int
    processing_time_ms: int


class BookUploadResponse(BaseModel):
    book: UploadedBookInfo
    blob: BlobInfo
    structure: StructureDetail
    meta: UploadMetaInfo
