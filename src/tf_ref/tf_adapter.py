from __future__ import annotations

from collections.abc import Iterable, Sequence
from typing import Any


class TextFabricAdapter:
    """Adapt an already-loaded Text-Fabric app to the reference core."""

    def __init__(self, tf_app: Any, *, corpus_id: str | None = None, lang: str = "en"):
        self.app = tf_app
        self.api = tf_app.api
        self._corpus_id = corpus_id
        self._lang = lang

    def corpus_id(self) -> str:
        if self._corpus_id:
            return self._corpus_id
        context = getattr(self.app, "context", None)
        for source in (context, self.app):
            for name in ("repo", "appName", "name"):
                value = getattr(source, name, None)
                if value:
                    return str(value)
        raise AttributeError("Text-Fabric app has no corpus identity")

    def section_types(self) -> tuple[str, ...]:
        return tuple(self.api.T.sectionTypes)

    def version(self) -> str:
        context = getattr(self.app, "context", None)
        return str(
            getattr(self.app, "version", None) or getattr(context, "version", None) or "latest"
        )

    def default_language(self) -> str:
        return self._lang

    def node_type(self, node: int) -> str:
        return str(self.api.F.otype.v(node))

    def node_from_section(self, values: Sequence[str], lang: str) -> int | None:
        try:
            typed = tuple(
                int(value) if kind == "int" else value
                for value, kind in zip(values, self.api.T.sectionFeatureTypes, strict=False)
            )
        except ValueError:
            return None
        return self.api.T.nodeFromSection(typed, lang=lang)

    def section_from_node(self, node: int, lang: str) -> tuple[str, ...]:
        return tuple(str(value) for value in self.api.T.sectionFromNode(node, lang=lang))

    def slots(self, node: int) -> tuple[int, ...]:
        if node <= self.api.F.otype.maxSlot:
            return (node,)
        return tuple(self.api.E.oslots.s(node))

    def units_of_type(self, otype: str) -> Iterable[int]:
        return self.api.F.otype.s(otype)
