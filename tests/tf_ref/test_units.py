from fake_corpus import FakeCorpus

from tf_ref import resolve, serialize


def test_spanning_unit_is_anchored_only_to_its_first_section():
    corpus = FakeCorpus()
    assert resolve("Vol1:3:4!clause1", corpus) == 3001
    assert resolve("Vol1:3:5!clause1", corpus) == 3002


def test_cache_scans_each_unit_type_once_for_repeated_serialization():
    corpus = FakeCorpus()
    for node in range(2001, 2009):
        serialize(node, corpus)
    assert corpus.unit_scans["word"] == 1
