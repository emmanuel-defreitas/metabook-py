from __future__ import annotations

from collections import defaultdict
from collections.abc import Iterable, Sequence


class FakeCorpus:
    """Small three-level corpus with one clause spanning two leaf sections."""

    def __init__(self, section_types: tuple[str, ...] = ("volume", "chapter", "paragraph")):
        self._section_types = section_types
        self.unit_scans: defaultdict[str, int] = defaultdict(int)
        self._paths = {
            "en": {
                100: ("Vol1",),
                110: ("Vol1", "3"),
                111: ("Vol1", "3", "4"),
                112: ("Vol1", "3", "5"),
            },
            "fr": {
                100: ("Tome1",),
                110: ("Tome1", "3"),
                111: ("Tome1", "3", "4"),
                112: ("Tome1", "3", "5"),
            },
        }
        self._slots = {
            100: tuple(range(1, 9)),
            110: tuple(range(1, 9)),
            111: (1, 2, 3, 4),
            112: (5, 6, 7, 8),
            **{2000 + slot: (slot,) for slot in range(1, 9)},
            3001: (3, 4, 5),
            3002: (6, 7),
        }
        self._types = {
            **{node: section_types[len(path) - 1] for node, path in self._paths["en"].items()},
            **{2000 + slot: "word" for slot in range(1, 9)},
            3001: "clause",
            3002: "clause",
        }

    def corpus_id(self) -> str:
        return "demo"

    def section_types(self) -> tuple[str, ...]:
        return self._section_types

    def version(self) -> str:
        return "2026"

    def default_language(self) -> str:
        return "en"

    def node_type(self, node: int) -> str:
        return self._types[node]

    def node_from_section(self, values: Sequence[str], lang: str) -> int | None:
        wanted = tuple(values)
        return next(
            (node for node, path in self._paths.get(lang, {}).items() if path == wanted), None
        )

    def section_from_node(self, node: int, lang: str) -> tuple[str, ...]:
        first_slot = min(self.slots(node))
        candidates = [
            (section, path)
            for section, path in self._paths.get(lang, {}).items()
            if first_slot in self._slots[section]
        ]
        if not candidates:
            return ()
        return max(candidates, key=lambda item: len(item[1]))[1]

    def slots(self, node: int) -> tuple[int, ...]:
        if 1 <= node <= 8:
            return (node,)
        return self._slots[node]

    def units_of_type(self, otype: str) -> Iterable[int]:
        self.unit_scans[otype] += 1
        return tuple(node for node, node_type in self._types.items() if node_type == otype)
