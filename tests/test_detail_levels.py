"""
Tests for the detail= nesting levels (sentence / clause / word nodes).

All deep nodes must carry only positions and counts — never text.
"""

import pytest

from metabook_py.services.counter import DETAIL_LEVELS, build_structure_tree
from metabook_py.services.detector import detect_schema
from metabook_py.services.fetcher import _normalise


def _chapters(raw: str, detail: str):
    text = _normalise(raw, is_html=False)
    schema = detect_schema(text)
    nodes, _ = build_structure_tree(text, schema, detail=detail)
    return nodes


def test_paragraph_detail_has_no_sentences(standard_book_raw):
    chapters = _chapters(standard_book_raw, "paragraph")
    assert all(p.sentences is None for c in chapters for p in c.paragraphs)


def test_sentence_detail_nests_sentences(standard_book_raw):
    chapters = _chapters(standard_book_raw, "sentence")
    first_para = chapters[0].paragraphs[0]
    assert first_para.sentences is not None
    assert len(first_para.sentences) == first_para.sentence_count
    first_sentence = first_para.sentences[0]
    assert first_sentence.index == 1
    assert first_sentence.word_count > 0
    assert first_sentence.clause_count >= 1
    assert first_sentence.clauses is None  # not requested at this level


def test_clause_detail_nests_clauses(standard_book_raw):
    chapters = _chapters(standard_book_raw, "clause")
    # "It is a truth universally acknowledged, that a single man …" has a comma
    first_sentence = chapters[0].paragraphs[0].sentences[0]
    assert first_sentence.clauses is not None
    assert len(first_sentence.clauses) == first_sentence.clause_count
    assert first_sentence.clauses[0].word_count > 0
    assert first_sentence.clauses[0].words is None  # not requested


def test_word_detail_nests_words(standard_book_raw):
    chapters = _chapters(standard_book_raw, "word")
    first_clause = chapters[0].paragraphs[0].sentences[0].clauses[0]
    assert first_clause.words is not None
    assert len(first_clause.words) == first_clause.word_count
    word = first_clause.words[0]
    # Purely positional — a word node must never contain text
    assert set(word.model_dump().keys()) == {"index"}
    assert word.index == 1


def test_clause_counts_are_consistent(standard_book_raw):
    chapters = _chapters(standard_book_raw, "word")
    for chapter in chapters:
        for para in chapter.paragraphs:
            for sentence in para.sentences:
                clause_words = sum(c.word_count for c in sentence.clauses)
                assert clause_words == sentence.word_count


def test_detail_levels_are_ordered():
    assert DETAIL_LEVELS == ("paragraph", "sentence", "clause", "word")


@pytest.mark.parametrize("detail", ["sentence", "clause", "word"])
def test_scripture_verses_gain_sentences(scripture_raw, detail):
    chapters = _chapters(scripture_raw, detail)
    verse = chapters[0].children[0].paragraphs[0]
    assert verse.sentences is not None and len(verse.sentences) >= 1


# ── Gutenberg-KJV formatting (titled books, no CHAPTER headings) ───────────────

KJV_STYLE_TEXT = """
*** START OF THE PROJECT GUTENBERG EBOOK ***

The Old Testament of the Bible
The First Book of Moses: Called Genesis
The Book of Job

The First Book of Moses: Called Genesis

1:1 In the beginning God created the heaven and the earth.

1:2 And the earth was without form, and void; and darkness was upon
the face of the deep. And God said, Let there be light: 1:3 And there
was light everywhere upon the face of the deep.

2:1 Thus the heavens and the earth were finished, and all the host of them.

2:2 And on the seventh day God ended his work which he had made.

The Book of Job

1:1 There was a man in the land of Uz, whose name was Job.

The words of Job are ended.

2:1 Again there was a day when the sons of God came to present themselves.

*** END OF THE PROJECT GUTENBERG EBOOK ***
"""


def test_kjv_style_books_and_verse_number_chapters():
    from metabook_py.services.detector import DetectedSchema, SchemaType

    text = _normalise(KJV_STYLE_TEXT, is_html=False)
    schema = DetectedSchema(SchemaType.CANONICAL_SCRIPTURE, "high", 10)
    chapters, _ = build_structure_tree(text, schema, detail="paragraph")
    # Two real books; the table of contents, the testament banner, and the
    # sentence "The words of Job are ended." must not become books.
    assert [b.label for b in chapters] == [
        "The First Book of Moses: Called Genesis",
        "The Book of Job",
    ]
    genesis = chapters[0]
    assert genesis.child_count == 2  # chapters derived from C:V numbers
    assert genesis.children[0].label == "Chapter 1"
    # 1:3 sits mid-line and must still be counted
    assert genesis.children[0].paragraph_count == 3
    assert [v.index for v in genesis.children[0].paragraphs] == [1, 2, 3]
    assert genesis.children[1].paragraph_count == 2
    job = chapters[1]
    assert job.child_count == 2
