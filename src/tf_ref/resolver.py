from __future__ import annotations

from .adapter import CorpusAdapter
from .errors import IndexOutOfRange, SectionNotFound, TypeNotInSection
from .grammar import format_ref, parse
from .reference import Reference
from .units import unit_index


def _parsed(ref: Reference | str, corpus: CorpusAdapter) -> Reference:
    if isinstance(ref, str):
        return parse(ref, depth=len(corpus.section_types()))
    if len(ref.sections) > len(corpus.section_types()):
        from .errors import ParseError

        raise ParseError(
            ref.sections[len(corpus.section_types())], "reference exceeds corpus depth"
        )
    return ref


def _section(ref: Reference, corpus: CorpusAdapter, lang: str) -> int:
    section_types = corpus.section_types()
    for depth in range(1, len(ref.sections) + 1):
        section = corpus.node_from_section(ref.sections[:depth], lang)
        if section is None:
            raise SectionNotFound(ref.sections[depth - 1], section_types[depth - 1])
    assert section is not None
    return section


def resolve(
    ref: Reference | str, corpus: CorpusAdapter, *, lang: str | None = None
) -> int | list[int]:
    parsed = _parsed(ref, corpus)
    language = lang or corpus.default_language()
    section = _section(parsed, corpus, language)
    if parsed.is_section:
        return section
    assert parsed.otype is not None and parsed.start is not None
    units = unit_index(corpus).units(section, parsed.otype, language)
    section_text = ":".join(parsed.sections)
    if not units:
        raise TypeNotInSection(parsed.otype, section_text)
    end = parsed.end or parsed.start
    if parsed.start > len(units) or end > len(units):
        requested = str(parsed.start) if parsed.end is None else f"{parsed.start}-{parsed.end}"
        raise IndexOutOfRange(requested, len(units))
    selected = units[parsed.start - 1 : end]
    return list(selected) if parsed.is_range else selected[0]


def serialize(
    node: int,
    corpus: CorpusAdapter,
    *,
    otype: str | None = None,
    corpus_id: str | None = None,
    lang: str | None = None,
    urn: bool = False,
) -> str:
    language = lang or corpus.default_language()
    node_type = otype or corpus.node_type(node)
    sections = corpus.section_from_node(node, language)
    if node_type in corpus.section_types():
        depth = corpus.section_types().index(node_type) + 1
        ref = Reference(corpus_id or corpus.corpus_id(), corpus.version(), sections[:depth])
    else:
        section = corpus.node_from_section(sections, language)
        if section is None:
            raise SectionNotFound(sections[-1], corpus.section_types()[-1])
        position = unit_index(corpus).position(section, node_type, node, language)
        if position is None:
            raise TypeNotInSection(node_type, ":".join(sections))
        ref = Reference(
            corpus_id or corpus.corpus_id(), corpus.version(), sections, node_type, position
        )
    return format_ref(ref, urn=urn)


def normalize(
    ref: Reference | str,
    corpus: CorpusAdapter,
    *,
    lang: str | None = None,
    urn: bool = False,
) -> str:
    parsed = _parsed(ref, corpus)
    normalized = Reference(
        parsed.corpus_id or corpus.corpus_id(),
        corpus.version(),
        parsed.sections,
        parsed.otype,
        parsed.start,
        parsed.end,
    )
    return format_ref(normalized, urn=urn)
