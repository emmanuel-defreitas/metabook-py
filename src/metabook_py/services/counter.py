"""
Counter service.

Responsibilities
----------------
- Split raw text into the structural tree implied by the detected schema.
- For every node in the tree, count child nodes, paragraphs, sentences,
  and words.
- Return (nodes, StructureSummary) — NO text content ever leaves this module.

Counting rules
--------------
- Paragraph  : blank-line–separated block of ≥ 10 characters.
- Sentence   : split on [.!?]['"»]? followed by whitespace + uppercase,
               with protection for common abbreviations.
- Word       : any token matching \b[a-zA-Z0-9]+\b.
"""

from __future__ import annotations

import re
from collections.abc import Callable

from metabook_py.models.structure import (
    ChapterNode,
    ClauseNode,
    ParagraphNode,
    PartNode,
    SentenceNode,
    StructureSummary,
    WordNode,
)
from metabook_py.services.detector import (
    CAPS_TITLE_RE,
    CHAPTER_NUM_RE,
    CHAPTER_WORD_RE,
    PART_RE,
    SCRIPTURE_BOOK_RE,
    VERSE_RE,
    DetectedSchema,
    SchemaType,
)

# Detail levels for the depth of leaf nesting, shallow → deep.
DETAIL_LEVELS = ("paragraph", "sentence", "clause", "word")

# ── Sentence-splitting ─────────────────────────────────────────────────────────

_ABBREVS = re.compile(
    r"\b(?:Mr|Mrs|Ms|Dr|Prof|Sr|Jr|Rev|Gen|Col|Maj|Sgt|Cpl|Lt|Cmdr|Adm|"
    r"Gov|Sen|Rep|St|Ave|Blvd|Dept|est|approx|vol|no|pp|fig|"
    r"Jan|Feb|Mar|Apr|Jun|Jul|Aug|Sep|Oct|Nov|Dec|"
    r"Inc|Ltd|Corp|Bros|Co|vs|etc|al|ibid|op|cit)\.",
    re.IGNORECASE,
)
_PLACEHOLDER = "<!P!>"
# Python's re module requires fixed-width look-behinds, so we match on the
# punctuation character itself (no trailing quote in look-behind).
# Sentences ending with ." or !' are still split correctly because the
# space after the closing quote triggers the look-behind on the quote itself
# — which is covered by the forward split on whitespace + uppercase.
_SENTENCE_BOUNDARY = re.compile(r"(?<=[.!?])\s+(?=[A-Z\"\u2018\u201C])")


def _split_sentences(text: str) -> list[str]:
    """Split a paragraph into sentences, protecting abbreviations."""
    protected = _ABBREVS.sub(lambda m: m.group(0).replace(".", _PLACEHOLDER), text)
    parts = _SENTENCE_BOUNDARY.split(protected)
    return [s.replace(_PLACEHOLDER, ".").strip() for s in parts if s.strip()]


def _count_words(text: str) -> int:
    return len(re.findall(r"\b[a-zA-Z0-9]+\b", text))


def _split_paragraphs(text: str) -> list[str]:
    """Return non-trivial paragraphs (at least 3 chars — filters blank lines and stray punctuation)."""
    return [p.strip() for p in re.split(r"\n\s*\n", text) if len(p.strip()) >= 3]


# ── Node builders ──────────────────────────────────────────────────────────────

# Clause boundaries: comma, semicolon, colon, and dashes.
_CLAUSE_BOUNDARY = re.compile(r"[,;:]|—|–|--")


def _word_nodes(text: str) -> list[WordNode]:
    return [WordNode(index=i + 1) for i in range(_count_words(text))]


def _clause_nodes(sentence: str, *, detail: str) -> list[ClauseNode]:
    clauses = [c.strip() for c in _CLAUSE_BOUNDARY.split(sentence) if c.strip()]
    return [
        ClauseNode(
            index=i + 1,
            word_count=_count_words(c),
            words=_word_nodes(c) if detail == "word" else None,
        )
        for i, c in enumerate(clauses)
    ]


def _sentence_nodes(sentences: list[str], *, detail: str) -> list[SentenceNode]:
    nodes = []
    for i, s in enumerate(sentences):
        clauses = _clause_nodes(s, detail=detail)
        nodes.append(
            SentenceNode(
                index=i + 1,
                clause_count=max(1, len(clauses)),
                word_count=_count_words(s),
                clauses=clauses if detail in ("clause", "word") else None,
            )
        )
    return nodes


def _para_node(text: str, index: int, *, detail: str = "paragraph") -> ParagraphNode:
    sentences = _split_sentences(text)
    sc = max(1, len(sentences))
    wc = _count_words(text)
    return ParagraphNode(
        index=index,
        sentence_count=sc,
        word_count=wc,
        avg_words_per_sentence=round(wc / sc, 2),
        sentences=_sentence_nodes(sentences, detail=detail) if detail != "paragraph" else None,
    )


def _chapter_node(
    text: str,
    index: int,
    label: str,
    level: str,
    *,
    include_paragraphs: bool,
    detail: str = "paragraph",
) -> ChapterNode:
    paras = _split_paragraphs(text)
    para_nodes = [_para_node(p, i + 1, detail=detail) for i, p in enumerate(paras)]

    if not para_nodes:
        return ChapterNode(
            level=level,
            index=index,
            label=label,
            paragraph_count=0,
            avg_sentences_per_paragraph=0.0,
            avg_words_per_sentence=0.0,
            total_words=0,
            total_sentences=0,
            paragraphs=[] if include_paragraphs else None,
        )

    total_s = sum(p.sentence_count for p in para_nodes)
    total_w = sum(p.word_count for p in para_nodes)
    pc = len(para_nodes)

    return ChapterNode(
        level=level,
        index=index,
        label=label,
        paragraph_count=pc,
        avg_sentences_per_paragraph=round(total_s / pc, 2),
        avg_words_per_sentence=round(total_w / max(total_s, 1), 2),
        total_words=total_w,
        total_sentences=total_s,
        paragraphs=para_nodes if include_paragraphs else None,
    )


# ── Generic "split text on regex, return (heading, content) pairs" ─────────────


def _split_on(text: str, pattern: re.Pattern) -> list[tuple[str, str]]:
    """
    Find all matches of *pattern* in *text* and return
    [(match_text, content_until_next_match), …].

    Content before the first match is discarded (it is usually front-matter).
    """
    matches = list(pattern.finditer(text))
    if not matches:
        return []
    result: list[tuple[str, str]] = []
    for i, m in enumerate(matches):
        heading = m.group(0).strip()
        start = m.end()
        end = matches[i + 1].start() if i + 1 < len(matches) else len(text)
        content = text[start:end].strip()
        result.append((heading, content))
    return result


# ── Schema-specific builders ───────────────────────────────────────────────────


def _build_flat(
    text: str, *, include_paragraphs: bool, detail: str = "paragraph"
) -> tuple[list[ParagraphNode], StructureSummary]:
    nodes = [_para_node(p, i + 1, detail=detail) for i, p in enumerate(_split_paragraphs(text))]
    total_s = sum(p.sentence_count for p in nodes)
    total_w = sum(p.word_count for p in nodes)
    pc = len(nodes)
    summary = StructureSummary(
        total_top_level_nodes=pc,
        total_paragraphs=pc,
        total_sentences=total_s,
        total_words=total_w,
        avg_paragraphs_per_chapter=1.0,
        avg_sentences_per_paragraph=round(total_s / max(pc, 1), 2),
        avg_words_per_sentence=round(total_w / max(total_s, 1), 2),
    )
    result: list[ParagraphNode] = nodes if include_paragraphs else []
    return result, summary


def _build_standard_book(
    text: str, schema: DetectedSchema, *, include_paragraphs: bool, detail: str = "paragraph"
) -> tuple[list[ChapterNode], StructureSummary]:
    # Prefer explicit "Chapter N" wording; fall back to standalone numerals
    splits = _split_on(text, CHAPTER_WORD_RE) or _split_on(text, CHAPTER_NUM_RE)
    if not splits:
        nodes, summary = _build_flat(text, include_paragraphs=include_paragraphs, detail=detail)
        return nodes, summary  # type: ignore[return-value]

    chapters = [
        _chapter_node(
            content,
            i + 1,
            label or f"Chapter {i + 1}",
            "chapter",
            include_paragraphs=include_paragraphs,
            detail=detail,
        )
        for i, (label, content) in enumerate(splits)
    ]
    return _chapters_summary(chapters)


def _build_sectioned_book(
    text: str, *, include_paragraphs: bool, detail: str = "paragraph"
) -> tuple[list[PartNode], StructureSummary]:
    part_splits = _split_on(text, PART_RE)
    if not part_splits:
        chapters, summary = _build_standard_book(
            text,
            DetectedSchema(SchemaType.SECTIONED_BOOK, "low", 0),
            include_paragraphs=include_paragraphs,
            detail=detail,
        )
        return chapters, summary  # type: ignore[return-value]

    parts: list[PartNode] = []
    for pi, (part_label, part_content) in enumerate(part_splits):
        ch_splits = _split_on(part_content, CHAPTER_WORD_RE) or _split_on(
            part_content, CHAPTER_NUM_RE
        )
        if not ch_splits:
            ch_splits = [("", part_content)]

        chapters = [
            _chapter_node(
                content,
                ci + 1,
                label or f"Chapter {ci + 1}",
                "chapter",
                include_paragraphs=include_paragraphs,
                detail=detail,
            )
            for ci, (label, content) in enumerate(ch_splits)
        ]
        total_pp = sum(c.paragraph_count for c in chapters)
        total_ww = sum(c.total_words for c in chapters)
        parts.append(
            PartNode(
                level="part",
                index=pi + 1,
                label=part_label or f"Part {pi + 1}",
                child_count=len(chapters),
                total_paragraphs=total_pp,
                total_words=total_ww,
                children=chapters,
            )
        )

    return _parts_summary(parts)


def _build_essay_collection(
    text: str, *, include_paragraphs: bool, detail: str = "paragraph"
) -> tuple[list[ChapterNode], StructureSummary]:
    splits = _split_on(text, CAPS_TITLE_RE)
    if not splits:
        nodes, summary = _build_flat(text, include_paragraphs=include_paragraphs, detail=detail)
        return nodes, summary  # type: ignore[return-value]

    essays = [
        _chapter_node(
            content,
            i + 1,
            label or f"Essay {i + 1}",
            "essay",
            include_paragraphs=include_paragraphs,
            detail=detail,
        )
        for i, (label, content) in enumerate(splits)
    ]
    return _chapters_summary(essays)


def _build_scripture(
    text: str, *, include_paragraphs: bool, detail: str = "paragraph"
) -> tuple[list[PartNode], StructureSummary]:
    """
    For scripture each 'paragraph' is a verse (identified by "N:N " patterns).
    Hierarchy: Book → Chapter → Verse-paragraph.
    """
    book_splits = _split_on(text, SCRIPTURE_BOOK_RE)
    if not book_splits:
        return _build_sectioned_book(text, include_paragraphs=include_paragraphs, detail=detail)

    books: list[PartNode] = []
    for bi, (book_label, book_content) in enumerate(book_splits):
        ch_splits = _split_on(book_content, CHAPTER_WORD_RE)
        if not ch_splits:
            ch_splits = [("", book_content)]

        chapters: list[ChapterNode] = []
        for ci, (ch_label, ch_content) in enumerate(ch_splits):
            # Treat each verse line as a leaf paragraph
            verse_texts = [
                v.strip() for v in VERSE_RE.split(ch_content) if len(v.strip()) >= 3
            ] or _split_paragraphs(ch_content)

            verse_nodes = [_para_node(v, vi + 1, detail=detail) for vi, v in enumerate(verse_texts)]
            total_s = sum(v.sentence_count for v in verse_nodes)
            total_w = sum(v.word_count for v in verse_nodes)
            vc = len(verse_nodes)
            chapters.append(
                ChapterNode(
                    level="chapter",
                    index=ci + 1,
                    label=ch_label or f"Chapter {ci + 1}",
                    paragraph_count=vc,
                    avg_sentences_per_paragraph=round(total_s / max(vc, 1), 2),
                    avg_words_per_sentence=round(total_w / max(total_s, 1), 2),
                    total_words=total_w,
                    total_sentences=total_s,
                    paragraphs=verse_nodes if include_paragraphs else None,
                )
            )

        total_bp = sum(c.paragraph_count for c in chapters)
        total_bw = sum(c.total_words for c in chapters)
        books.append(
            PartNode(
                level="book",
                index=bi + 1,
                label=book_label or f"Book {bi + 1}",
                child_count=len(chapters),
                total_paragraphs=total_bp,
                total_words=total_bw,
                children=chapters,
            )
        )

    return _parts_summary(books)


# ── Summary helpers ────────────────────────────────────────────────────────────


def _chapters_summary(
    chapters: list[ChapterNode],
) -> tuple[list[ChapterNode], StructureSummary]:
    total_p = sum(c.paragraph_count for c in chapters)
    total_s = sum(c.total_sentences for c in chapters)
    total_w = sum(c.total_words for c in chapters)
    nc = len(chapters)
    summary = StructureSummary(
        total_top_level_nodes=nc,
        total_paragraphs=total_p,
        total_sentences=total_s,
        total_words=total_w,
        avg_paragraphs_per_chapter=round(total_p / max(nc, 1), 2),
        avg_sentences_per_paragraph=round(total_s / max(total_p, 1), 2),
        avg_words_per_sentence=round(total_w / max(total_s, 1), 2),
    )
    return chapters, summary


def _parts_summary(
    parts: list[PartNode],
) -> tuple[list[PartNode], StructureSummary]:
    total_chapters = sum(p.child_count for p in parts)
    total_p = sum(p.total_paragraphs for p in parts)
    total_w = sum(p.total_words for p in parts)
    total_s = sum(c.total_sentences for p in parts for c in p.children)
    np = len(parts)
    summary = StructureSummary(
        total_top_level_nodes=np,
        total_mid_level_nodes=total_chapters,
        total_paragraphs=total_p,
        total_sentences=total_s,
        total_words=total_w,
        avg_paragraphs_per_chapter=round(total_p / max(total_chapters, 1), 2),
        avg_sentences_per_paragraph=round(total_s / max(total_p, 1), 2),
        avg_words_per_sentence=round(total_w / max(total_s, 1), 2),
    )
    return parts, summary


# ── Public API ─────────────────────────────────────────────────────────────────


def build_structure_tree(
    text: str,
    schema: DetectedSchema,
    *,
    include_paragraphs: bool = True,
    detail: str = "paragraph",
) -> tuple[list, StructureSummary]:
    """
    Build and return (nodes, summary) for *text* according to *schema*.
    Nodes are Pydantic model instances — no raw text is included anywhere.

    *detail* controls leaf nesting depth (see DETAIL_LEVELS): paragraphs may
    contain sentences, sentences clauses, and clauses words. Deeper levels
    still carry only positions and counts (a word node is index + length).
    """
    dispatch: dict[SchemaType, Callable[[], tuple[list, StructureSummary]]] = {
        SchemaType.CANONICAL_SCRIPTURE: lambda: _build_scripture(
            text, include_paragraphs=include_paragraphs, detail=detail
        ),
        SchemaType.SECTIONED_BOOK: lambda: _build_sectioned_book(
            text, include_paragraphs=include_paragraphs, detail=detail
        ),
        SchemaType.STANDARD_BOOK: lambda: _build_standard_book(
            text, schema, include_paragraphs=include_paragraphs, detail=detail
        ),
        SchemaType.ESSAY_COLLECTION: lambda: _build_essay_collection(
            text, include_paragraphs=include_paragraphs, detail=detail
        ),
        SchemaType.FLAT: lambda: _build_flat(
            text, include_paragraphs=include_paragraphs, detail=detail
        ),
    }
    builder = dispatch.get(schema.name, dispatch[SchemaType.FLAT])
    return builder()
