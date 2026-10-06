from types import SimpleNamespace

from fake_corpus import FakeCorpus

from tf_ref import TextFabricAdapter, node_to_ref, resolve_ref


def tf_app():
    corpus = FakeCorpus()

    class Text:
        sectionTypes = corpus.section_types()  # noqa: N815 - external API name
        sectionFeatureTypes = ("str", "int", "int")  # noqa: N815 - external API name

        def nodeFromSection(self, values, *, lang):  # noqa: N802 - external API name
            return corpus.node_from_section(tuple(map(str, values)), lang)

        def sectionFromNode(self, node, *, lang):  # noqa: N802 - external API name
            return corpus.section_from_node(node, lang)

    otype = SimpleNamespace(
        maxSlot=8,
        v=corpus.node_type,
        s=lambda kind: corpus.units_of_type(kind),
    )
    api = SimpleNamespace(
        T=Text(),
        F=SimpleNamespace(otype=otype),
        E=SimpleNamespace(oslots=SimpleNamespace(s=corpus.slots)),
    )
    return SimpleNamespace(api=api, version="2026", context=SimpleNamespace(repo="demo"))


def test_adapter_and_legacy_wrappers_use_the_core_behavior():
    app = tf_app()
    adapter = TextFabricAdapter(app)
    assert adapter.node_from_section(("Vol1", "3", "4"), "en") == 111
    assert resolve_ref("Vol1:3:4!word2", app) == 2002
    assert node_to_ref(2002, app) == "demo@2026/Vol1:3:4!word2"
    assert node_to_ref(2002, app, corpus_id="custom") == "custom@2026/Vol1:3:4!word2"
