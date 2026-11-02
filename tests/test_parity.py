"""
Parity: METABOOK_PARSER=rust must produce output identical to the Python
implementation. Skipped when the metabook_core extension isn't installed
(`uv sync --extra rust`).
"""

from __future__ import annotations

import json
import random
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

metabook_core = pytest.importorskip("metabook_core")

from metabook_py.core.config import settings  # noqa: E402
from metabook_py.services import textstats  # noqa: E402
from metabook_py.services.counter import DETAIL_LEVELS, build_structure_tree  # noqa: E402
from metabook_py.services.detector import detect_schema  # noqa: E402
from metabook_py.services.documents.service import analyze_document  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "tests" / "fixtures" / "documents"
KJV = ROOT / ".cache" / "bench" / "pg10.txt"

sys.path.insert(0, str(ROOT / "tests"))
import conftest  # noqa: E402

BOOK_TEXTS = {k: v for k, v in vars(conftest).items() if k.isupper() and isinstance(v, str)}
CONTENT_TYPES = {
    ".md": "text/markdown",
    ".html": "text/html",
    ".txt": "text/plain",
    ".pdf": "application/pdf",
}


def _run(parser: str, fn, *args, **kwargs):
    old = settings.metabook_parser
    settings.metabook_parser = parser  # type: ignore[assignment]
    try:
        return fn(*args, **kwargs)
    finally:
        settings.metabook_parser = old  # type: ignore[assignment]


def _doc(parser: str, data: bytes, content_type: str) -> dict:
    result = _run(parser, analyze_document, data, content_type=content_type)
    payload = result.model_dump(mode="json")
    assert payload["meta"]["parser"] == parser
    return payload["structure"]


def assert_doc_parity(data: bytes, content_type: str) -> None:
    assert _doc("rust", data, content_type) == _doc("python", data, content_type)


# ── Single documents ───────────────────────────────────────────────────────────


@pytest.mark.parametrize("path", sorted(DOCS.glob("paper.*")), ids=lambda p: p.name)
def test_fixture_parity(path):
    assert_doc_parity(path.read_bytes(), CONTENT_TYPES[path.suffix])


EDGE_CASES = [
    ("text/markdown", b""),
    ("text/plain", b"   \n\n\t"),
    ("text/markdown", b"No headings. Two sentences.\n\nAnother."),
    ("text/plain", b"No headings. Two sentences.\n\nAnother."),
    ("text/markdown", b"# Title\n\n```\nunclosed fence\n# not a heading"),
    ("text/markdown", b"### deep first\n\ntext\n\n# top\n\n## mid\n\n#### skip"),
    (
        "text/markdown",
        b"* * *\n\n- a\n  continued\n\n  - nested [x](u) ![i](p)\n- b\n\n1) one\n2) two",
    ),
    (
        "text/markdown",
        b"> quote\nlazy line\n> # quoted heading\n\n[ref]: http://x\n\n[t][ref] <http://a.b> www",
    ),
    (
        "text/markdown",
        b"Setext\n===\n\nSub\n---\n\n    indented code\n\n|a|b|\n|-|-|\n|[x](y)|z|\n\n[^n]: note\n    more",
    ),
    (
        "text/markdown",
        b"---\nfront: matter\n---\n# H #\n\n## References\n\n- Doe. 2020.\n\n## Notes\n\npara",
    ),
    (
        "text/plain",
        b"TITLE CASE\n\n1. Intro\n\n2.3.1 Deep Part\n\n- a\n  - b\n    - c\n- d\n\nx | y\n1 | 2",
    ),
    (
        "text/plain",
        b"> quoted. Two sentences.\n> More.\n\n[^1]: Footnote https://x.y/z.\n\nFig. 2 caption",
    ),
    ("text/plain", "Unicode été. “Quoted” next. Mr. X　Y. a\x1cB. Done!".encode()),
    ("text/html", b"<h1>Title<p>Unclosed. Two.<ul><li>item</ul></li></div><h3>Next</h3><p>x"),
    (
        "text/html",
        b"<p>Loose <a href='#'>link</a>.</p><blockquote>Bare quote text. Two.</blockquote>",
    ),
]


@pytest.mark.parametrize(("ctype", "data"), EDGE_CASES)
def test_edge_case_parity(ctype, data):
    assert_doc_parity(data, ctype)


# Lines that exercise every adapter rule; documents are random mixes of them.
_LINES = [
    "# Heading one", "## Two", "### Three", "###### Six", "#NoSpace", "Setext", "===", "---",
    "- item", "* item", "+ item", "  - nested", "    - deeper", "1. first", "2) second",
    "a. alpha", "• bullet", "> quote", "> > nested quote", "```", "~~~", "    code",
    "| a | b |", "|---|---|", "x | y", "a\tb", "[^1]: note", "[ref]: http://e.x",
    "![img](p.png)", "Figure 3. Caption", "Fig. 4 caption", "REFERENCES", "Notes",
    "1.2 Numbered Heading", "Dr. Smith arrived. He sat down.", "Mr. Jones et al. wrote it.",
    "See [link](http://a.b) and <https://c.d> or https://e.f/g.", "Odd ![x [y](z) case.",
    "What? Yes! “Quoted.” ‘Single.’ \"Double.\"", "trailing etc. Next one.",
    "café naïve x² ١٢", "  ", "", "\t", "word\x1cWord. Next\x1dLine.",
]  # fmt: skip


@pytest.mark.parametrize("seed", range(40))
@pytest.mark.parametrize("ctype", ["text/markdown", "text/plain", "text/html"])
def test_fuzz_parity(seed, ctype):
    rng = random.Random(seed)
    lines = [rng.choice(_LINES) for _ in range(rng.randint(5, 60))]
    if ctype == "text/html":
        tags = ["p", "h1", "h3", "li", "blockquote", "td", "figure", "pre", "div"]
        lines = [f"<{t}>{line}</{t}>" for line, t in ((ln, rng.choice(tags)) for ln in lines)]
    assert_doc_parity("\n".join(lines).encode(), ctype)


# ── Shared tokenizer ───────────────────────────────────────────────────────────


@pytest.mark.parametrize("seed", range(20))
def test_tokenizer_parity(seed):
    rng = random.Random(seed)
    alphabet = list('aZ .!?\n\t"‘“\x1c,;:-') + ["Mr.", "Dr.", "etc.", "U.S.", "é", "5"]
    text = "".join(rng.choice(alphabet) for _ in range(400))
    assert metabook_core.split_sentences(text) == textstats._split_sentences_py(text)
    assert metabook_core.count_words(text) == len(textstats._WORD.findall(text))


# ── Books (shared tokenizer under the book counter) ────────────────────────────


@pytest.mark.parametrize("name", sorted(BOOK_TEXTS))
@pytest.mark.parametrize("detail", DETAIL_LEVELS)
def test_book_parity(name, detail):
    text = BOOK_TEXTS[name]

    def build():
        schema = detect_schema(text)
        nodes, summary = build_structure_tree(text, schema, detail=detail)
        return [n.model_dump() for n in nodes], summary.model_dump()

    assert _run("rust", build) == _run("python", build)


@pytest.mark.skipif(not KJV.exists(), reason="KJV not cached (see scripts/bench_parser.py)")
def test_kjv_parity():
    text = KJV.read_text(encoding="utf-8")

    def build():
        schema = detect_schema(text)
        nodes, summary = build_structure_tree(text, schema)
        return json.dumps([n.model_dump() for n in nodes]), summary.model_dump()

    assert _run("rust", build) == _run("python", build)


# ── CLI ────────────────────────────────────────────────────────────────────────

_CLI = ROOT / "rust" / "metabook_core" / "target" / "release" / "metabook"


@pytest.mark.skipif(
    not _CLI.exists() and shutil.which("metabook") is None,
    reason="CLI not built (cargo build --release in rust/metabook_core)",
)
@pytest.mark.parametrize("name", ["paper.md", "paper.txt"])
def test_cli_matches_api(name):
    cli = str(_CLI) if _CLI.exists() else "metabook"
    out = subprocess.run([cli, "analyze", str(DOCS / name)], capture_output=True, check=True)
    payload = json.loads(out.stdout)
    assert payload["meta"]["parser"] == "rust"
    expected = _doc("python", (DOCS / name).read_bytes(), CONTENT_TYPES[Path(name).suffix])
    assert payload["structure"] == expected


@pytest.mark.skipif(not _CLI.exists(), reason="CLI not built")
def test_cli_rejects_html():
    out = subprocess.run([str(_CLI), "analyze", str(DOCS / "paper.html")], capture_output=True)
    assert out.returncode == 2 and b"unsupported" in out.stderr
