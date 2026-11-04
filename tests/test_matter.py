"""Explicit excluded matter and opt-in word token counts preserve text privacy."""

from unittest.mock import AsyncMock, patch

import pytest

from metabook_py.core.cache import book_text_cache
from metabook_py.services.counter import _word_nodes
from metabook_py.services.fetcher import fetch_book_text
from metabook_py.services.preprocessing import detect_matter, preprocess_text

SOURCE = "Publisher header\n*** START OF THE PROJECT GUTENBERG EBOOK TEST ***\nChapter I\n\nAuthored body text.\n*** END OF THE PROJECT GUTENBERG EBOOK TEST ***\nLicense footer"


def test_explicit_matter_is_separate_and_contains_no_prose():
    regions = detect_matter(SOURCE)
    assert [item["position"] for item in regions] == ["front", "back"]
    assert regions[0]["end"] < regions[1]["start"]
    assert all(item["word_count"] > 0 for item in regions)
    assert "Publisher header" not in str(regions)
    body = preprocess_text(SOURCE)
    assert "Authored body text." in body
    assert "Publisher header" not in body
    assert "License footer" not in body


def test_authored_prefaces_and_afterwords_are_not_guessed():
    assert (
        detect_matter(
            "Preface\nAn author's introduction.\nChapter I\nStory.\nAfterword\nMore writing."
        )
        == []
    )


@pytest.mark.asyncio
async def test_cached_text_keeps_matter_without_refetching():
    book_text_cache.clear()
    with patch(
        "metabook_py.services.fetcher._rate_limited_get", new=AsyncMock(return_value=SOURCE)
    ) as fetch:
        first, cached = await fetch_book_text(991, "https://example.com/book.txt")
        second, cached_again = await fetch_book_text(991, "https://example.com/book.txt")
    assert not cached and cached_again
    assert first == second
    assert first.matter == second.matter == detect_matter(SOURCE)
    fetch.assert_awaited_once()
    book_text_cache.clear()


def test_words_have_individual_counts_only_when_requested():
    words = _word_nodes("One, longer-word!", encoder=len)
    assert [word.token_count for word in words] == [3, 6, 4]
    assert [word.index for word in words] == [1, 2, 3]
    assert _word_nodes("One")[0].model_dump() == {"index": 1}
    assert all(set(word.model_dump()) == {"index", "token_count"} for word in words)


def test_crlf_matter_offsets_refer_to_original_source():
    source = SOURCE.replace("\n", "\r\n")
    regions = detect_matter(source)
    assert len(regions) == 2
    assert source[regions[1]["start"]:].startswith("*** END OF")
    assert source[:regions[0]["end"]].endswith("***")
