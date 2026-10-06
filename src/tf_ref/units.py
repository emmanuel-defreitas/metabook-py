from __future__ import annotations

from collections import defaultdict
from weakref import WeakKeyDictionary

from .adapter import CorpusAdapter


class UnitIndex:
    def __init__(self, corpus: CorpusAdapter):
        self.corpus = corpus
        self._built: set[tuple[str, str]] = set()
        self._lists: dict[tuple[str, int, str], tuple[int, ...]] = {}
        self._positions: dict[tuple[str, int, str, int], int] = {}

    def _build(self, otype: str, lang: str) -> None:
        by_section: defaultdict[int, list[int]] = defaultdict(list)
        for node in self.corpus.units_of_type(otype):
            slots = self.corpus.slots(node)
            if not slots:
                continue
            path = self.corpus.section_from_node(min(slots), lang)
            for depth in range(1, len(path) + 1):
                section = self.corpus.node_from_section(path[:depth], lang)
                if section is not None:
                    by_section[section].append(node)
        for section, nodes in by_section.items():
            values = tuple(nodes)
            self._lists[(lang, section, otype)] = values
            self._positions.update(
                {(lang, section, otype, node): index for index, node in enumerate(values, 1)}
            )
        self._built.add((lang, otype))

    def units(self, section: int, otype: str, lang: str) -> tuple[int, ...]:
        if (lang, otype) not in self._built:
            self._build(otype, lang)
        return self._lists.get((lang, section, otype), ())

    def position(self, section: int, otype: str, node: int, lang: str) -> int | None:
        self.units(section, otype, lang)
        return self._positions.get((lang, section, otype, node))


_INDEXES: WeakKeyDictionary[object, UnitIndex] = WeakKeyDictionary()


def unit_index(corpus: CorpusAdapter) -> UnitIndex:
    index = _INDEXES.get(corpus)
    if index is None:
        index = UnitIndex(corpus)
        _INDEXES[corpus] = index
    return index
