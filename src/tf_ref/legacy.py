from __future__ import annotations

from typing import Any

from .resolver import resolve, serialize
from .tf_adapter import TextFabricAdapter


def resolve_ref(ref_str: str, tf_app: Any) -> int | list[int]:
    return resolve(ref_str, TextFabricAdapter(tf_app))


def node_to_ref(node: int, tf_app: Any, corpus_id: str | None = None) -> str:
    return serialize(node, TextFabricAdapter(tf_app), corpus_id=corpus_id)
