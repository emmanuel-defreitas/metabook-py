import pytest
from fake_corpus import FakeCorpus

from tf_ref import format_ref, normalize, parse, resolve, serialize


@pytest.mark.parametrize(
    ("text", "normalized"),
    [
        ("Vol1", "demo@2026/Vol1"),
        ("Vol1:3", "demo@2026/Vol1:3"),
        ("Vol1:3:4!word2", "demo@2026/Vol1:3:4!word2"),
        ("demo@old/Vol1:3:4!clause1", "demo@2026/Vol1:3:4!clause1"),
    ],
)
def test_normalization_is_version_explicit_and_idempotent(text, normalized):
    corpus = FakeCorpus()
    assert normalize(text, corpus) == normalized
    assert normalize(normalized, corpus) == normalized


@pytest.mark.parametrize(
    "text",
    ["Vol1", "Vol1:3", "Vol1:3:4", "Vol1:3:4!word2", "Vol1:3:4!clause1"],
)
def test_single_node_resolve_serialize_matches_normalize(text):
    corpus = FakeCorpus()
    assert serialize(resolve(text, corpus), corpus) == normalize(text, corpus)


def test_partial_depth_selector_serializes_to_its_canonical_leaf_section():
    corpus = FakeCorpus()
    node = resolve("Vol1:3!word5", corpus)
    assert serialize(node, corpus) == "demo@2026/Vol1:3:5!word1"


def test_range_endpoints_serialize_to_the_requested_bounds():
    corpus = FakeCorpus()
    nodes = resolve("Vol1:3:4!word2-4", corpus)
    assert isinstance(nodes, list)
    assert serialize(nodes[0], corpus).endswith("!word2")
    assert serialize(nodes[-1], corpus).endswith("!word4")


def test_spanning_unit_serializes_to_the_section_containing_its_first_slot():
    corpus = FakeCorpus()
    assert serialize(3001, corpus) == "demo@2026/Vol1:3:4!clause1"
    assert serialize(3002, corpus) == "demo@2026/Vol1:3:5!clause1"


def test_serialization_uses_requested_heading_language():
    corpus = FakeCorpus()
    assert serialize(2002, corpus, lang="fr") == "demo@2026/Tome1:3:4!word2"


def test_short_and_urn_forms_are_lossless_with_reserved_heading_characters():
    short = 'demo@2026/"Part One":"Intro: Notes":"say ""hi"""!word2-3'
    urn = format_ref(parse(short), urn=True)
    assert urn == "urn:tf:demo@2026:Part%20One:Intro%3A%20Notes:say%20%22hi%22!word2-3"
    assert format_ref(parse(urn)) == short
    assert format_ref(parse(format_ref(parse(urn))), urn=True) == urn
