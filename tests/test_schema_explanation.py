"""Regression and public-contract coverage using only synthetic text."""

import base64
from pathlib import Path
from types import SimpleNamespace

import httpx
import pytest
from conftest import build_epub

from metabook_py.main import app
from metabook_py.mcp_server import search_book_structure, upload_book_epub
from metabook_py.models.book import BookInfo
from metabook_py.services.counter import build_structure_tree
from metabook_py.services.detector import SchemaType, detect_schema
from metabook_py.services.fetcher import _normalise
from metabook_py.services.preprocessing import preprocess_text

FIXTURES = Path(__file__).parent / "fixtures"


@pytest.mark.parametrize(
    "filename,expected,chapters",
    [
        ("contents_chapters.txt", SchemaType.STANDARD_BOOK, 2),
        ("contents_flat.txt", SchemaType.FLAT, 0),
    ],
)
def test_contents_removed_before_detection_and_counting(filename, expected, chapters):
    raw = (FIXTURES / filename).read_text()
    clean = preprocess_text(raw)
    assert "CONTENTS" not in clean
    assert "...." not in clean
    assert preprocess_text(clean) == clean
    schema = detect_schema(raw)
    assert schema.name == expected
    assert schema.evidence["chapter_word_markers"] == chapters
    _, actual = build_structure_tree(raw, schema)
    _, wanted = build_structure_tree(clean, detect_schema(clean))
    assert actual == wanted
    assert actual.total_paragraphs == 2
    assert actual.total_words == 21


@pytest.mark.parametrize(
    "text",
    [
        "CHAPTER I\n\nThe first chapter contains actual prose. It must remain.\n\nCONTENTS\n\nCHAPTER II\nCHAPTER III\n\nCHAPTER II\n\nThe second chapter remains too.",
        "CONTENTS\n\nThis is an essay about the contents of an old box. It is real prose.",
        "CONTENTS\n\nCHAPTER I\nCHAPTER II",  # no confirmed body
        "PREFACE\n\nThis introduction belongs to the author. Keep this prose.\n\nCHAPTER I\n\nThe chapter begins.",
        "The narrator mentions *** END OF THE PROJECT GUTENBERG EBOOK *** inside prose.",
    ],
)
def test_uncertain_frontmatter_and_real_prose_preserved(text):
    assert preprocess_text(text) == text


@pytest.mark.parametrize(
    "start,end",
    [
        (
            "*** START OF THE PROJECT GUTENBERG EBOOK SAMPLE ***",
            "*** END OF THE PROJECT GUTENBERG EBOOK SAMPLE ***",
        ),
        (
            "***START OF THIS PROJECT GUTENBERG ETEXT SAMPLE***",
            "***END OF THIS PROJECT GUTENBERG ETEXT SAMPLE***",
        ),
        (
            "*** start of project gutenberg ebook\nSAMPLE ***",
            "*** end of project gutenberg ebook\nSAMPLE ***",
        ),
        (
            "*** START OF THE PROJECT GUTENBERG EBOOK SAMPLE ***",
            "End of Project Gutenberg's EBook Sample, by Author",
        ),
    ],
)
def test_license_variants(start, end):
    body = "A traveller waited at the river.\n\nThe boat arrived."
    raw = f"License and header\r\n{start}\r\n\r\n{body}\r\n\r\n{end}\r\nFull license with CHAPTER I and CHAPTER II"
    assert _normalise(raw, is_html=False) == body
    assert detect_schema(raw).evidence["chapter_word_markers"] == 0
    _, summary = build_structure_tree(raw, detect_schema(raw))
    assert summary.total_words == 9


@pytest.mark.parametrize(
    "text",
    [
        "",
        "Ordinary prose without markers.",
        "CHAPTER I\n\nOnly one chapter exists.",
        "1:1 A single numbered line.",
    ],
)
def test_weak_evidence_is_not_high_confidence(text):
    result = detect_schema(text)
    assert result.name == SchemaType.FLAT
    assert result.confidence == "low"
    assert result.score <= 0.2
    assert set(result.candidate_scores) == {s.value for s in SchemaType}
    assert all(isinstance(count, int) for count in result.evidence.values())


def test_ambiguous_detection_exposes_competing_support():
    chapters = "\n\n".join(f"CHAPTER {i}\n\nA chapter contains prose." for i in range(1, 7))
    essays = "\n\nFIRST ESSAY\n\nSome prose.\n\nSECOND ESSAY\n\nOther prose."
    result = detect_schema(chapters + essays)
    assert result.name == SchemaType.STANDARD_BOOK
    assert result.confidence == "medium"
    assert result.score == 0.6
    assert result.candidate_scores["standard_book"] == 0.9
    assert result.candidate_scores["essay_or_story_collection"] == 0.5


def test_title_byline_pairs_strengthen_collection_support():
    plain = "\n\n".join(
        f"ESSAY {word}\n\nSome original prose." for word in ["ONE", "TWO", "THREE", "FOUR", "FIVE"]
    )
    bylines = plain.replace("\n\nSome", "\n\nBy Example Author\n\nSome")
    weak, strong = detect_schema(plain), detect_schema(bylines)
    assert strong.name == SchemaType.ESSAY_COLLECTION
    assert strong.evidence["title_byline_pairs"] == 5
    assert strong.score > weak.score
    assert strong.confidence == "high"


def test_override_preserves_automatic_evidence():
    text = "CHAPTER I\n\nOriginal prose.\n\nCHAPTER II\n\nMore original prose."
    automatic = detect_schema(text)
    override = detect_schema(text, schema_override=SchemaType.FLAT)
    assert override.name == SchemaType.FLAT
    assert override.detected_name == automatic.name
    assert override.evidence == automatic.evidence
    assert override.score == automatic.score
    assert override.overridden


@pytest.fixture
def fake_services(monkeypatch):
    text = (FIXTURES / "contents_chapters.txt").read_text()

    async def fetch(*args, **kwargs):
        return text, False

    async def search(*args, **kwargs):
        return (
            BookInfo(gutenberg_id=1, title="Sample", authors=[], language="en", subjects=[]),
            "https://example.com/book",
            False,
        )

    async def blob(*args, **kwargs):
        return SimpleNamespace(
            url="https://example.com/book.epub", pathname="books/book.epub", size=100
        )

    for module in ["metabook_py.routers.books", "metabook_py.mcp_server"]:
        monkeypatch.setattr(f"{module}.fetch_book_text", fetch)
        monkeypatch.setattr(f"{module}.upload_epub", blob)
    monkeypatch.setattr("metabook_py.services.discovery.GutendexClient.search", search)
    return text


def assert_explanation(structure):
    assert structure["schema"] == "flat"
    assert structure["schema_detected"] == "standard_book"
    assert structure["schema_overridden"] is True
    assert structure["schema_score"] == (
        0.6 if structure["schema_evidence"]["chapter_word_markers"] == 3 else 0.5
    )
    assert structure["schema_confidence"] == "medium"
    assert structure["schema_evidence"]["chapter_word_markers"] >= 2
    assert "standard_book" in structure["schema_candidates"]


@pytest.mark.parametrize("upload", [False, True])
async def test_rest_and_mcp_expose_explanation_and_override(fake_services, upload):
    async with httpx.AsyncClient(
        transport=httpx.ASGITransport(app=app), base_url="http://test"
    ) as client:
        if upload:
            resp = await client.post(
                "/api/books/upload?schema_override=flat",
                files={"file": ("book.epub", build_epub(), "application/epub+zip")},
            )
            result = await upload_book_epub(
                epub_base64=base64.b64encode(build_epub()).decode(), schema_override=SchemaType.FLAT
            )
        else:
            resp = await client.get("/api/books/structure?gutenberg_id=1&schema_override=flat")
            result = await search_book_structure(gutenberg_id=1, schema_override=SchemaType.FLAT)
        assert resp.status_code in (200, 201)
        assert_explanation(resp.json()["structure"])
        assert_explanation(result["structure"])
        # The explanation cannot expose source excerpts or positional spans.
        assert all(isinstance(v, int) for v in result["structure"]["schema_evidence"].values())


async def test_invalid_override_rejected_before_fetch(fake_services):
    async with httpx.AsyncClient(
        transport=httpx.ASGITransport(app=app), base_url="http://test"
    ) as client:
        response = await client.get("/api/books/structure?gutenberg_id=1&schema_override=wrong")
        assert response.status_code == 422
    result = await search_book_structure(gutenberg_id=1, schema_override="wrong")
    assert result["error"] == "invalid_schema_override"


@pytest.mark.parametrize(
    "table",
    [
        "CHAPTER I\n\nCHAPTER II\n\nCHAPTER III",
        "CHAPTER I\nCHAPTER II\nCHAPTER III",
    ],
)
def test_last_contents_entry_does_not_become_a_body_chapter(table):
    prose = "This is the first real paragraph. It has ordinary prose."
    text = f"CONTENTS\n\n{table}\n\n{prose}"
    assert preprocess_text(text) == prose
    assert detect_schema(text).name == SchemaType.FLAT


def test_repeated_heading_after_spaced_contents_is_preserved():
    body = "CHAPTER I\n\nReal prose begins here. It must be preserved."
    text = f"CONTENTS\n\nCHAPTER I\n\nCHAPTER II\n\nCHAPTER III\n\n{body}"
    assert preprocess_text(text) == body


@pytest.mark.parametrize(
    "text,name,signal,count",
    [
        (
            "\n".join(f"1:{i} Original verse." for i in range(1, 11)),
            SchemaType.CANONICAL_SCRIPTURE,
            "verse_number_lines",
            10,
        ),
        (
            "PART I\n\nCHAPTER I\n\nOriginal prose.\n\nPART II\n\nCHAPTER II\n\nOther prose.",
            SchemaType.SECTIONED_BOOK,
            "part_markers",
            2,
        ),
        (
            "I.\n\nOriginal prose.\n\nII.\n\nOther prose.",
            SchemaType.STANDARD_BOOK,
            "chapter_numeral_markers",
            2,
        ),
    ],
)
def test_schema_specific_evidence(text, name, signal, count):
    result = detect_schema(text)
    assert result.name == name
    assert result.evidence[signal] == count
    assert result.candidate_scores[name.value] >= 0.5


def test_caps_title_pattern_does_not_join_adjacent_lines():
    text = "FIRST TITLE\nSECOND TITLE\n\nOriginal prose."
    assert detect_schema(text).evidence["caps_title_lines"] == 2


def test_compact_html_preserves_body_boundaries_and_strips_license():
    raw = (
        "<p>License header</p>"
        "<p>*** START OF THE PROJECT GUTENBERG EBOOK SAMPLE ***</p>"
        "<h2>CONTENTS</h2><p>CHAPTER I .... 1<br>CHAPTER II .... 2</p>"
        "<h2>CHAPTER I</h2><p>Original prose belongs to chapter one.</p>"
        "<h2>CHAPTER II</h2><p>Original prose belongs to chapter two.</p>"
        "<p>*** END OF THE PROJECT GUTENBERG EBOOK SAMPLE ***</p>"
        "<p>License footer</p>"
    )
    clean = _normalise(raw, is_html=True)
    assert clean.startswith("CHAPTER I")
    assert "License" not in clean
    assert "CONTENTS" not in clean
    result = detect_schema(clean)
    assert result.evidence["chapter_word_markers"] == 2
    _, summary = build_structure_tree(clean, result)
    assert summary.total_words == 12
