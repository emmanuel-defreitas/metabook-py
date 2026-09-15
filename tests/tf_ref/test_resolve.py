import pytest
from fake_corpus import FakeCorpus

from tf_ref import IndexOutOfRange, SectionNotFound, TypeNotInSection, resolve


@pytest.mark.parametrize("levels", [("volume", "chapter", "paragraph"), ("book", "scene", "line")])
def test_full_and_partial_sections_resolve_without_level_names(levels):
    corpus = FakeCorpus(levels)
    assert resolve("Vol1:3:4", corpus) == 111
    assert resolve("Vol1:3", corpus) == 110


def test_missing_section_names_heading_and_level():
    corpus = FakeCorpus()
    with pytest.raises(SectionNotFound, match="missing.*paragraph"):
        resolve("Vol1:3:missing", corpus)


def test_selector_and_inclusive_range_resolve_in_canonical_order():
    corpus = FakeCorpus()
    assert resolve("Vol1:3:4!word2", corpus) == 2002
    assert resolve("Vol1:3:4!word2-4", corpus) == [2002, 2003, 2004]


def test_selector_errors_are_distinct_and_actionable():
    corpus = FakeCorpus()
    with pytest.raises(TypeNotInSection, match="sentence.*Vol1:3:4"):
        resolve("Vol1:3:4!sentence1", corpus)
    with pytest.raises(IndexOutOfRange, match=r"5.*1\.\.4"):
        resolve("Vol1:3:4!word5", corpus)
