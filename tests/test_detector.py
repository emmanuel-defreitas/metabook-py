"""Tests for services/detector.py"""

from metabook_py.services.detector import SchemaType, detect_schema
from metabook_py.services.fetcher import _normalise


def norm(raw: str) -> str:
    return _normalise(raw, is_html=False)


class TestBoilerplateStripping:
    def test_strips_pg_header_and_footer(self, standard_book_raw):
        text = norm(standard_book_raw)
        assert "PROJECT GUTENBERG" not in text

    def test_content_preserved(self, standard_book_raw):
        text = norm(standard_book_raw)
        assert "CHAPTER I" in text
        assert "truth universally acknowledged" in text


class TestStandardBook:
    def test_detects_schema(self, standard_book_raw):
        schema = detect_schema(norm(standard_book_raw))
        assert schema.name == SchemaType.STANDARD_BOOK

    def test_confidence(self, standard_book_raw):
        schema = detect_schema(norm(standard_book_raw))
        # 3 chapters → medium or high
        assert schema.confidence in ("medium", "high")

    def test_markers_found(self, standard_book_raw):
        schema = detect_schema(norm(standard_book_raw))
        assert schema.markers_found >= 3


class TestSectionedBook:
    def test_detects_schema(self, sectioned_book_raw):
        schema = detect_schema(norm(sectioned_book_raw))
        assert schema.name == SchemaType.SECTIONED_BOOK

    def test_markers_include_parts_and_chapters(self, sectioned_book_raw):
        schema = detect_schema(norm(sectioned_book_raw))
        # 2 parts + 3 chapters = 5 markers
        assert schema.markers_found >= 4
        assert schema.confidence in ("medium", "high")


class TestEssayCollection:
    def test_detects_schema(self, essay_collection_raw):
        schema = detect_schema(norm(essay_collection_raw))
        assert schema.name == SchemaType.ESSAY_COLLECTION

    def test_markers_found(self, essay_collection_raw):
        schema = detect_schema(norm(essay_collection_raw))
        assert schema.markers_found >= 2


class TestFlat:
    def test_detects_schema(self, flat_raw):
        schema = detect_schema(norm(flat_raw))
        assert schema.name == SchemaType.FLAT

    def test_confidence_is_high(self, flat_raw):
        # Flat fallback always reports high confidence
        schema = detect_schema(norm(flat_raw))
        assert schema.confidence == "high"


class TestScripture:
    def test_detects_schema(self, scripture_raw):
        schema = detect_schema(norm(scripture_raw))
        assert schema.name == SchemaType.CANONICAL_SCRIPTURE

    def test_many_verse_markers(self, scripture_raw):
        schema = detect_schema(norm(scripture_raw))
        assert schema.markers_found >= 10


class TestConfidenceLevels:
    def test_high_confidence_threshold(self):
        # 5+ identical markers → high
        text = "\n".join([f"CHAPTER {i}\n\nSome text here.\n" for i in range(1, 7)])
        schema = detect_schema(text)
        assert schema.confidence == "high"

    def test_medium_confidence_threshold(self):
        text = "CHAPTER I\n\nText.\n\nCHAPTER II\n\nMore text.\n\nCHAPTER III\n\nFinal."
        schema = detect_schema(text)
        assert schema.name == SchemaType.STANDARD_BOOK
        assert schema.confidence in ("medium", "high")
