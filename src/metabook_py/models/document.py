"""
Response models for single-document analysis.

See docs/single-document-schema.md. Counts and positions only — no text.
"""

from __future__ import annotations

from datetime import datetime
from typing import Literal

from pydantic import BaseModel, Field, HttpUrl

from metabook_py.models.structure import ParagraphNode

SCHEMA_VERSION = "1.0"

DocumentFormat = Literal["html", "markdown", "pdf", "text"]
ParserName = Literal["python", "rust"]


class Counts(BaseModel):
    headings: int = 0
    paragraphs: int = 0
    lists: int = 0
    list_items: int = 0
    tables: int = 0
    figures: int = 0
    code_blocks: int = 0
    blockquotes: int = 0
    footnotes: int = 0
    references: int = 0
    links: int = 0
    sentences: int = 0
    words: int = 0
    clauses: int | None = Field(default=None, exclude_if=lambda value: value is None)

    def __add__(self, other: Counts) -> Counts:
        return Counts(**{
            k: (getattr(self, k) or 0) + (getattr(other, k) or 0)
            for k in Counts.model_fields
            if getattr(self, k) is not None or getattr(other, k) is not None
        })


class DocumentTotals(Counts):
    sections: int = 0
    max_depth: int = 0


class SectionNode(BaseModel):
    kind: Literal["preamble", "section"]
    level: int = Field(ge=0, le=6, description="0 for the preamble, else the heading level")
    index: int = Field(ge=0, description="Ordinal among headed siblings; preamble is 0")
    path: str = Field(description='Dotted ordinal path, e.g. "2.1.3"; preamble is "0"')
    counts: Counts = Field(description="Own content incl. heading, excluding subsections")
    totals: Counts = Field(description="counts plus every descendant section")
    children: list[SectionNode] = Field(default_factory=list)
    paragraphs: list[ParagraphNode] | None = Field(default=None, exclude_if=lambda value: value is None)


class DocumentStructure(BaseModel):
    totals: DocumentTotals
    sections: list[SectionNode]


class DocumentInfo(BaseModel):
    format: DocumentFormat
    content_type: str
    size_bytes: int
    sha256: str
    page_count: int | None = None
    filename: str | None = None
    source_url: str | None = None


class DocumentMeta(BaseModel):
    schema_version: str = SCHEMA_VERSION
    parser: ParserName
    processing_time_ms: int
    analyzed_at: datetime


class DocumentAnalysisResponse(BaseModel):
    document: DocumentInfo
    structure: DocumentStructure
    meta: DocumentMeta


class AnalyzeUrlRequest(BaseModel):
    url: HttpUrl
