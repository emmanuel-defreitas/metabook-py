"""Tests for services/counter.py"""

from metabook_py.services.counter import (
    _count_words,
    _split_paragraphs,
    _split_sentences,
    build_structure_tree,
)
from metabook_py.services.detector import detect_schema
from metabook_py.services.fetcher import _normalise


def norm(raw: str) -> str:
    return _normalise(raw, is_html=False)


# ── Sentence splitter ──────────────────────────────────────────────────────────


class TestSentenceSplitter:
    def test_basic_three_sentences(self):
        text = "This is one. This is two. This is three."
        assert len(_split_sentences(text)) == 3

    def test_question_and_exclamation(self):
        text = "Is this right? Yes it is! Great."
        assert len(_split_sentences(text)) == 3

    def test_mr_abbreviation_not_split(self):
        text = "Mr. Smith went to see Dr. Jones in Washington. He arrived late."
        parts = _split_sentences(text)
        assert len(parts) == 2

    def test_single_sentence(self):
        text = "Just one sentence here with no period"
        parts = _split_sentences(text)
        assert len(parts) == 1

    def test_empty_string(self):
        assert _split_sentences("") == []

    def test_month_abbreviations_protected(self):
        text = "She was born Jan. 5th in the city. He arrived Feb. 3rd."
        parts = _split_sentences(text)
        assert len(parts) == 2


# ── Word counter ───────────────────────────────────────────────────────────────


class TestWordCounter:
    def test_basic_count(self):
        assert _count_words("hello world") == 2

    def test_punctuation_excluded(self):
        assert _count_words("hello, world!") == 2

    def test_empty(self):
        assert _count_words("") == 0

    def test_numbers_counted(self):
        assert _count_words("Chapter 1 has 5 words total") == 6

    def test_hyphenated(self):
        # "well-known" → two tokens
        assert _count_words("a well-known fact") == 4


# ── Paragraph splitter ─────────────────────────────────────────────────────────


class TestParagraphSplitter:
    def test_three_paragraphs(self):
        text = "First para.\n\nSecond para.\n\nThird para."
        assert len(_split_paragraphs(text)) == 3

    def test_multiple_blank_lines_collapsed(self):
        text = "First.\n\n\n\n\nSecond."
        assert len(_split_paragraphs(text)) == 2

    def test_short_fragments_excluded(self):
        # Fragments < 10 chars are ignored
        text = "Real paragraph here.\n\n.\n\nAnother real paragraph here."
        assert len(_split_paragraphs(text)) == 2


# ── Full tree building ─────────────────────────────────────────────────────────


class TestBuildFlat:
    def test_paragraph_count(self, flat_raw):
        text = norm(flat_raw)
        schema = detect_schema(text)
        nodes, summary = build_structure_tree(text, schema)
        assert summary.total_paragraphs == 3
        assert summary.total_words > 0
        assert summary.avg_words_per_sentence > 0


class TestBuildStandardBook:
    def test_chapter_count(self, standard_book_raw):
        text = norm(standard_book_raw)
        schema = detect_schema(text)
        nodes, summary = build_structure_tree(text, schema)
        assert summary.total_top_level_nodes == 3  # 3 CHAPTER markers

    def test_paragraphs_present_in_chapters(self, standard_book_raw):
        text = norm(standard_book_raw)
        schema = detect_schema(text)
        nodes, summary = build_structure_tree(text, schema)
        assert all(n.paragraph_count > 0 for n in nodes)

    def test_no_text_in_nodes(self, standard_book_raw):
        text = norm(standard_book_raw)
        schema = detect_schema(text)
        nodes, _ = build_structure_tree(text, schema)
        for chapter in nodes:
            assert not hasattr(chapter, "text")
            if chapter.paragraphs:
                for para in chapter.paragraphs:
                    assert not hasattr(para, "text")

    def test_include_paragraphs_false(self, standard_book_raw):
        text = norm(standard_book_raw)
        schema = detect_schema(text)
        nodes, _ = build_structure_tree(text, schema, include_paragraphs=False)
        assert all(n.paragraphs is None for n in nodes)

    def test_summary_totals_consistent(self, standard_book_raw):
        text = norm(standard_book_raw)
        schema = detect_schema(text)
        nodes, summary = build_structure_tree(text, schema)
        assert summary.total_paragraphs == sum(n.paragraph_count for n in nodes)
        assert summary.total_words == sum(n.total_words for n in nodes)


class TestBuildSectionedBook:
    def test_part_and_chapter_counts(self, sectioned_book_raw):
        text = norm(sectioned_book_raw)
        schema = detect_schema(text)
        nodes, summary = build_structure_tree(text, schema)
        assert summary.total_top_level_nodes == 2  # 2 PART markers
        assert summary.total_mid_level_nodes == 3  # 3 CHAPTER markers total

    def test_children_present(self, sectioned_book_raw):
        text = norm(sectioned_book_raw)
        schema = detect_schema(text)
        nodes, _ = build_structure_tree(text, schema)
        assert all(len(part.children) > 0 for part in nodes)


class TestBuildEssayCollection:
    def test_essay_count(self, essay_collection_raw):
        text = norm(essay_collection_raw)
        schema = detect_schema(text)
        nodes, summary = build_structure_tree(text, schema)
        assert summary.total_top_level_nodes == 2

    def test_essay_level_label(self, essay_collection_raw):
        text = norm(essay_collection_raw)
        schema = detect_schema(text)
        nodes, _ = build_structure_tree(text, schema)
        assert all(n.level == "essay" for n in nodes)


class TestBuildScripture:
    def test_book_count(self, scripture_raw):
        text = norm(scripture_raw)
        schema = detect_schema(text)
        nodes, summary = build_structure_tree(text, schema)
        assert summary.total_top_level_nodes >= 1  # Genesis + Exodus
        assert summary.total_mid_level_nodes is not None

    def test_verse_counts(self, scripture_raw):
        text = norm(scripture_raw)
        schema = detect_schema(text)
        nodes, summary = build_structure_tree(text, schema)
        assert summary.total_paragraphs > 0
        assert summary.total_words > 0
