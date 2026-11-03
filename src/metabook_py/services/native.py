"""
Parser backend selection (METABOOK_PARSER=python|rust).

"rust" uses the metabook_core extension (rust/metabook_core, built with
maturin) when it is installed. If it is requested but missing, the Python
implementation is used and a warning is logged once — callers report the
backend that actually ran via active_parser().
"""

from __future__ import annotations

import logging
from functools import cache
from types import ModuleType

from metabook_py.core.config import settings

log = logging.getLogger(__name__)


@cache
def _load_core() -> ModuleType | None:
    try:
        import metabook_core  # type: ignore[import-not-found]
    except ImportError:
        log.warning("METABOOK_PARSER=rust but metabook_core is not installed; using Python")
        return None
    return metabook_core


def native_core() -> ModuleType | None:
    """The Rust extension module when selected and available, else None."""
    if settings.metabook_parser != "rust":
        return None
    return _load_core()


def active_parser() -> str:
    return "rust" if native_core() is not None else "python"
