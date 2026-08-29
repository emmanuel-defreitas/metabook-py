"""
Unit tests for the EPUB parsing service (services/epub.py).

An EPUB is built in memory with zipfile so no fixture binaries are needed.
"""

import zipfile
from io import BytesIO

import pytest
from conftest import CONTAINER_XML, OPF_TEMPLATE, build_epub

from metabook_py.core.exceptions import InvalidEpubError
from metabook_py.services.detector import SchemaType, detect_schema
from metabook_py.services.epub import parse_epub

# ── Happy path ─────────────────────────────────────────────────────────────────


def test_parses_metadata():
    parsed = parse_epub(build_epub())
    assert parsed.metadata.title == "Pride and Prejudice"
    assert parsed.metadata.authors == ["Jane Austen"]
    assert parsed.metadata.language == "en"
    assert parsed.metadata.subjects == ["Fiction", "Romance"]
    assert parsed.metadata.isbn == "9780141439518"


def test_extracts_spine_text_in_order():
    parsed = parse_epub(build_epub())
    assert parsed.spine_document_count == 3
    assert parsed.text.index("CHAPTER I") < parsed.text.index("CHAPTER II")
    assert "truth universally acknowledged" in parsed.text
    # style/head content must not leak
    assert "color: red" not in parsed.text
    assert "ignored" not in parsed.text


def test_headings_land_on_their_own_lines_for_the_detector():
    parsed = parse_epub(build_epub())
    schema = detect_schema(parsed.text)
    assert schema.name == SchemaType.STANDARD_BOOK


def test_missing_mimetype_is_tolerated():
    parsed = parse_epub(build_epub(with_mimetype=False))
    assert parsed.metadata.title == "Pride and Prejudice"


# ── Failure modes ──────────────────────────────────────────────────────────────


def test_rejects_non_zip():
    with pytest.raises(InvalidEpubError, match="zip"):
        parse_epub(b"definitely not a zip file")


def test_rejects_wrong_mimetype():
    with pytest.raises(InvalidEpubError, match="mimetype"):
        parse_epub(build_epub(mimetype="text/plain"))


def test_rejects_zip_without_container():
    buf = BytesIO()
    with zipfile.ZipFile(buf, "w") as zf:
        zf.writestr("mimetype", "application/epub+zip")
        zf.writestr("random.txt", "hello")
    with pytest.raises(InvalidEpubError, match="container.xml"):
        parse_epub(buf.getvalue())


def test_rejects_epub_with_no_text():
    buf = BytesIO()
    with zipfile.ZipFile(buf, "w") as zf:
        zf.writestr("mimetype", "application/epub+zip")
        zf.writestr("META-INF/container.xml", CONTAINER_XML)
        zf.writestr(
            "OEBPS/content.opf",
            OPF_TEMPLATE.format(manifest="", spine=""),
        )
    with pytest.raises(InvalidEpubError, match="No extractable text"):
        parse_epub(buf.getvalue())
