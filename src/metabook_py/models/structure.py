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

from pydantic import BaseModel, Field, model_serializer

from metabook_py.models.book import BlobInfo, BookInfo, UploadedBookInfo

# Tokenizer-derived fields are opt-in (present only when a ?tokenizer= was
# requested). When absent they are omitted from the serialized output entirely
# — not emitted as null — so responses without a tokenizer stay identical to
# the pre-token-count contract.
_TOKEN_FIELDS = ("token_count", "total_tokens", "tokenizer")


class _OmitAbsentTokenFields(BaseModel):
    @model_serializer(mode="wrap")
    def _omit_absent_token_fields(self, handler):
        data = handler(self)
        if isinstance(data, dict):
            for key in _TOKEN_FIELDS:
                if key in data and data[key] is None:
                    del data[key]
        return data


# ── Deep-detail nodes (opt-in via detail=sentence|clause|word) ─────────────────
#
# Like every other node these carry only positions and counts — never text.
# A word node is purely positional.


class WordNode(BaseModel):
    index: int


class ClauseNode(_OmitAbsentTokenFields):
    index: int
    word_count: int
    token_count: int | None = None  # None unless a tokenizer was requested
    words: list[WordNode] | None = None  # None unless detail="word"


class SentenceNode(_OmitAbsentTokenFields):
    index: int
    clause_count: int
    word_count: int
    token_count: int | None = None  # None unless a tokenizer was requested
    clauses: list[ClauseNode] | None = None  # None unless detail>="clause"


# ── Leaf node ──────────────────────────────────────────────────────────────────


class ParagraphNode(_OmitAbsentTokenFields):
    index: int
    sentence_count: int
    word_count: int
    token_count: int | None = None  # None unless a tokenizer was requested
    avg_words_per_sentence: float
    sentences: list[SentenceNode] | None = None  # None unless detail>="sentence"


# ── Mid-level node (chapter / essay / story) ───────────────────────────────────


class ChapterNode(_OmitAbsentTokenFields):
    level: str  # "chapter" | "essay" | "story"
    index: int
    label: str
    paragraph_count: int
    avg_sentences_per_paragraph: float
    avg_words_per_sentence: float
    total_words: int
    total_sentences: int
    total_tokens: int | None = None  # None unless a tokenizer was requested
    paragraphs: list[ParagraphNode] | None = None  # None when include_paragraphs=False


# ── Top-level node (part / volume / section / book) ────────────────────────────


class PartNode(_OmitAbsentTokenFields):
    level: str  # "part" | "volume" | "section" | "book"
    index: int
    label: str
    child_count: int
    total_paragraphs: int
    total_words: int
    total_tokens: int | None = None  # None unless a tokenizer was requested
    children: list[ChapterNode]


# ── Aggregates ─────────────────────────────────────────────────────────────────


class StructureSummary(_OmitAbsentTokenFields):
    total_top_level_nodes: int
    total_mid_level_nodes: int | None = None  # None for flat / standard_book
    total_paragraphs: int
    total_sentences: int
    total_words: int
    total_tokens: int | None = None  # None unless a tokenizer was requested
    avg_paragraphs_per_chapter: float
    avg_sentences_per_paragraph: float
    avg_words_per_sentence: float


# ── Tokenizer echo ─────────────────────────────────────────────────────────────
#
# A token count is never reported without the scheme that produced it: any
# response carrying token counts also carries this block in its metadata.


class TokenizerInfo(BaseModel):
    name: str
    vocab_size: int


# ── Top-level response ─────────────────────────────────────────────────────────


class StructureDetail(BaseModel):
    schema_type: str = Field(alias="schema")
    schema_confidence: str  # "high" | "medium" | "low"
    summary: StructureSummary
    nodes: list[Any]  # list[PartNode | ChapterNode | ParagraphNode]

    model_config = {"populate_by_name": True}


class MetaInfo(_OmitAbsentTokenFields):
    fetched_at: datetime
    cached: bool
    processing_time_ms: int
    tokenizer: TokenizerInfo | None = None  # None unless a tokenizer was requested


class BookStructureResponse(BaseModel):
    book: BookInfo
    structure: StructureDetail
    meta: MetaInfo


# ── Upload response ────────────────────────────────────────────────────────────


class UploadMetaInfo(_OmitAbsentTokenFields):
    uploaded_at: datetime
    spine_document_count: int
    processing_time_ms: int
    tokenizer: TokenizerInfo | None = None  # None unless a tokenizer was requested


class BookUploadResponse(BaseModel):
    book: UploadedBookInfo
    blob: BlobInfo
    structure: StructureDetail
    meta: UploadMetaInfo
