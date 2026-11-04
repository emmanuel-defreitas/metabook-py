"""
Single-document analysis: adapters, structure builder, endpoints, MCP tool.

Fixture counts in EXPECTED were counted by hand from tests/fixtures/documents/.
"""

from __future__ import annotations

import base64
import importlib.util
import json
from pathlib import Path

import httpx
import pytest
import respx

from metabook_py.core.config import settings
from metabook_py.core.exceptions import (
    InvalidDocumentError,
    UnsupportedDocumentTypeError,
)
from metabook_py.main import app
from metabook_py.mcp_server import analyze_document as mcp_analyze_document
from metabook_py.services.counter import _count_words, _para_node
from metabook_py.services.documents import fetch as doc_fetch
from metabook_py.services.documents.blocks import Block, BlockType, heading
from metabook_py.services.documents.builder import build_document_structure
from metabook_py.services.documents.service import analyze_document, resolve_format
from metabook_py.services.native import active_parser

FIXTURES = Path(__file__).parent / "fixtures" / "documents"


def _load_pdf_builder():
    spec = importlib.util.spec_from_file_location("make_pdf", FIXTURES / "make_pdf.py")
    module = importlib.util.module_from_spec(spec)  # type: ignore[arg-type]
    spec.loader.exec_module(module)  # type: ignore[union-attr]
    return module.build_pdf


build_pdf = _load_pdf_builder()
CONTENT_TYPES = {
    "paper.md": "text/markdown",
    "paper.html": "text/html",
    "paper.txt": "text/plain",
    "paper.pdf": "application/pdf",
}

_ZERO = dict.fromkeys(
    [
        "headings", "paragraphs", "lists", "list_items", "tables", "figures", "code_blocks",
        "blockquotes", "footnotes", "references", "links", "sentences", "words",
        "sections", "max_depth",
    ],
    0,
)  # fmt: skip


def _expect(**kw: int) -> dict[str, int]:
    return {**_ZERO, **kw}


# Hand-counted totals per fixture.
EXPECTED = {
    "paper.md": _expect(
        headings=7, paragraphs=5, lists=4, list_items=6, tables=1, figures=1, code_blocks=1,
        blockquotes=1, footnotes=1, references=2, links=3, sentences=17, words=76,
        sections=8, max_depth=3,
    ),
    "paper.html": _expect(
        headings=7, paragraphs=5, lists=5, list_items=6, tables=1, figures=1, code_blocks=1,
        blockquotes=1, footnotes=1, references=2, links=3, sentences=17, words=73,
        sections=8, max_depth=3,
    ),
    "paper.txt": _expect(
        headings=7, paragraphs=5, lists=3, list_items=6, tables=1, figures=1,
        blockquotes=1, footnotes=1, references=2, links=2, sentences=17, words=80,
        sections=8, max_depth=3,
    ),
    "paper.pdf": _expect(
        headings=5, paragraphs=4, lists=1, list_items=3, figures=1, references=2,
        sentences=12, words=51, sections=6, max_depth=3,
    ),
}  # fmt: skip

# (kind, level, path) in document order, per fixture.
EXPECTED_OUTLINE = {
    "paper.md": [
        ("preamble", 0, "0"),
        ("section", 1, "1"),
        ("section", 2, "1.1"),
        ("section", 3, "1.1.1"),
        ("section", 2, "1.2"),
        ("section", 4, "1.2.1"),  # skipped level 3 nests directly under level 2
        ("section", 2, "1.3"),
        ("section", 2, "1.4"),
    ],
    "paper.pdf": [
        ("preamble", 0, "0"),
        ("section", 1, "1"),
        ("section", 2, "1.1"),
        ("section", 3, "1.1.1"),
        ("section", 2, "1.2"),
        ("section", 2, "1.3"),
    ],
}
EXPECTED_OUTLINE["paper.html"] = EXPECTED_OUTLINE["paper.md"]
EXPECTED_OUTLINE["paper.txt"] = EXPECTED_OUTLINE["paper.md"]

# Metadata fields allowed to exceed the 64-character string limit.
LONG_STRING_ALLOWLIST = {"filename", "source_url", "content_type", "sha256", "analyzed_at"}


def _analyze(name: str):
    return analyze_document(
        (FIXTURES / name).read_bytes(), content_type=CONTENT_TYPES[name], filename=name
    )


def _outline(nodes) -> list[tuple[str, int, str]]:
    out = []
    for n in nodes:
        out.append((n.kind, n.level, n.path))
        out.extend(_outline(n.children))
    return out


def _long_strings(value, key: str | None = None) -> list[tuple[str | None, str]]:
    """Every string field longer than 64 characters outside the allowlist."""
    if isinstance(value, dict):
        return [hit for k, v in value.items() for hit in _long_strings(v, k)]
    if isinstance(value, list):
        return [hit for v in value for hit in _long_strings(v, key)]
    if isinstance(value, str) and len(value) > 64 and key not in LONG_STRING_ALLOWLIST:
        return [(key, value)]
    return []


@pytest.fixture
def public_dns(monkeypatch):
    """Resolve every host to a public address (no real DNS in tests)."""
    answers: dict[str, list[str]] = {}

    async def resolve(host: str) -> list[str]:
        return answers.get(host, ["93.184.216.34"])

    monkeypatch.setattr(doc_fetch, "resolve_host", resolve)
    return answers


# ── Fixtures: totals and outline ───────────────────────────────────────────────


@pytest.mark.parametrize("name", sorted(EXPECTED))
def test_fixture_totals(name):
    assert _analyze(name).structure.totals.model_dump() == EXPECTED[name]


@pytest.mark.parametrize("name", sorted(EXPECTED_OUTLINE))
def test_fixture_outline(name):
    assert _outline(_analyze(name).structure.sections) == EXPECTED_OUTLINE[name]


@pytest.mark.parametrize("name", sorted(EXPECTED))
def test_totals_are_sum_of_sections(name):
    result = _analyze(name)

    def check(node):
        summed = node.counts.model_dump()
        for child in node.children:
            check(child)
            for k, v in child.totals.model_dump().items():
                summed[k] += v
        assert node.totals.model_dump() == summed

    root = dict.fromkeys(EXPECTED[name], 0)
    for node in result.structure.sections:
        check(node)
        for k, v in node.totals.model_dump().items():
            root[k] += v
    totals = result.structure.totals.model_dump()
    assert {k: totals[k] for k in root if k not in ("sections", "max_depth")} == {
        k: v for k, v in root.items() if k not in ("sections", "max_depth")
    }


def test_markdown_section_counts():
    intro = _analyze("paper.md").structure.sections[1].children[0]
    counts = intro.counts
    assert (counts.paragraphs, counts.lists, counts.list_items, counts.sentences, counts.words) == (
        1,
        2,
        4,
        7,
        27,
    )
    assert intro.totals.words == 27 + 7  # plus the Background subsection


def test_pdf_reports_page_count():
    assert _analyze("paper.pdf").document.page_count == 2


# ── Structure builder ──────────────────────────────────────────────────────────


def _p(text: str = "One sentence here.") -> Block:
    return Block(BlockType.PARAGRAPH, text=text)


def test_skipped_levels_nest_directly_and_close_correctly():
    blocks = [heading(1, "A"), heading(3, "A.x"), _p(), heading(2, "B"), heading(4, "B.x")]
    sections, totals = build_document_structure(blocks)
    assert _outline(sections) == [
        ("section", 1, "1"),
        ("section", 3, "1.1"),
        ("section", 2, "1.2"),
        ("section", 4, "1.2.1"),
    ]
    assert totals.max_depth == 3


def test_deeper_heading_first_then_shallower_is_sibling():
    sections, _ = build_document_structure([heading(3, "deep"), _p(), heading(1, "top"), _p()])
    assert _outline(sections) == [("section", 3, "1"), ("section", 1, "2")]


def test_preamble_only_when_content_precedes_heading():
    sections, _ = build_document_structure([heading(1, "A"), _p()])
    assert [n.kind for n in sections] == ["section"]
    sections, _ = build_document_structure([_p(), heading(1, "A")])
    assert [(n.kind, n.level, n.index) for n in sections] == [
        ("preamble", 0, 0),
        ("section", 1, 1),
    ]


def test_headings_inside_containers_do_not_open_sections():
    quote = Block(BlockType.BLOCKQUOTE, children=[heading(1, "quoted"), _p()])
    sections, totals = build_document_structure([quote])
    assert [n.kind for n in sections] == ["preamble"]
    assert totals.headings == 1 and totals.sections == 1


def test_references_and_notes_headings_retype_content():
    blocks = [
        heading(2, "3. References"),
        _p("Smith, J. A paper. 2020."),
        Block(BlockType.LIST, children=[Block(BlockType.LIST_ITEM, text="Doe, A. 2021.")]),
        heading(2, "Endnotes"),
        _p("A note."),
    ]
    _, totals = build_document_structure(blocks)
    assert (totals.references, totals.footnotes, totals.paragraphs, totals.list_items) == (
        2,
        1,
        0,
        0,
    )
    assert totals.words == 2  # footnote prose counts; references do not


# ── Edge cases per format ──────────────────────────────────────────────────────

ALL_FORMATS = [
    ("text/markdown", "x.md"),
    ("text/html", "x.html"),
    ("text/plain", "x.txt"),
]


@pytest.mark.parametrize(("ctype", "filename"), ALL_FORMATS)
@pytest.mark.parametrize("body", [b"", b"   \n\n  \t\n"])
def test_empty_documents(ctype, filename, body):
    result = analyze_document(body, content_type=ctype, filename=filename)
    assert result.structure.sections == []
    assert result.structure.totals.model_dump() == _ZERO


def test_empty_pdf_page():
    result = analyze_document(build_pdf([[]]), content_type="application/pdf")
    assert result.structure.sections == []
    assert result.document.page_count == 1


@pytest.mark.parametrize(
    ("ctype", "body"),
    [
        ("text/markdown", b"First paragraph. Two sentences.\n\nSecond one."),
        ("text/html", b"<p>First paragraph. Two sentences.</p><p>Second one.</p>"),
        ("text/plain", b"First paragraph. Two sentences.\n\nSecond one."),
    ],
)
def test_no_headings_is_one_preamble(ctype, body):
    result = analyze_document(body, content_type=ctype)
    [preamble] = result.structure.sections
    assert (preamble.kind, preamble.level, preamble.path) == ("preamble", 0, "0")
    assert (preamble.counts.paragraphs, preamble.counts.sentences, preamble.counts.words) == (
        2,
        3,
        6,
    )


def test_no_headings_pdf():
    pdf = build_pdf([[(11, 700, "Only body text. Two sentences.")]])
    [preamble] = analyze_document(pdf, content_type="application/pdf").structure.sections
    assert preamble.kind == "preamble" and preamble.counts.sentences == 2


def test_sentence_and_word_counts_match_book_tokenizer():
    text = 'Mr. Brown met Dr. Who at 5 p.m. on St. James Ave. "Really?" she asked. Yes, etc. Fine!'
    book = _para_node(text, 1)
    result = analyze_document(text.encode(), content_type="text/plain")
    totals = result.structure.totals
    assert (totals.sentences, totals.words) == (book.sentence_count, book.word_count)
    assert totals.words == _count_words(text)


def test_malformed_html_is_tolerated():
    html = (
        b"<h1>Title<p>Unclosed paragraph. Two sentences.<ul><li>item</ul></li></div></span><h2>Next"
    )
    result = analyze_document(html, content_type="text/html")
    totals = result.structure.totals
    assert totals.headings == 2 and totals.list_items == 1 and totals.paragraphs == 1


def test_unclosed_markdown_fence_runs_to_end():
    md = b"# Title\n\n```\ncode\n\n# not a heading\n"
    totals = analyze_document(md, content_type="text/markdown").structure.totals
    assert (totals.headings, totals.code_blocks) == (1, 1)


@pytest.mark.parametrize(
    "body",
    [
        b"not a pdf at all",
        b"%PDF-1.4\n garbage without objects",
        b"%PDF-1.4\n1 0 obj << /Type /Catalog /Pages 9 0 R >> endobj\ntrailer << /Root 1 0 R >>\n%%EOF",
    ],
)
def test_malformed_pdf_raises(body):
    with pytest.raises(InvalidDocumentError):
        analyze_document(body, content_type="application/pdf")


def test_binary_in_text_document_raises():
    with pytest.raises(InvalidDocumentError):
        analyze_document(b"hello\x00\x01\x02world", content_type="text/plain")


@pytest.mark.parametrize(
    ("ctype", "name", "fmt"),
    [
        ("text/html; charset=utf-8", None, "html"),
        ("TEXT/MARKDOWN", None, "markdown"),
        ("application/octet-stream", "notes.MD", "markdown"),
        (None, "paper.pdf", "pdf"),
        ("", "https://x.test/a/readme.txt?v=1", "text"),
    ],
)
def test_resolve_format(ctype, name, fmt):
    assert resolve_format(ctype, name)[0] == fmt


@pytest.mark.parametrize(
    ("ctype", "name"),
    [("image/png", "x.png"), ("application/octet-stream", "x.docx"), (None, None)],
)
def test_unsupported_types(ctype, name):
    with pytest.raises(UnsupportedDocumentTypeError):
        resolve_format(ctype, name)


# ── Never return text ──────────────────────────────────────────────────────────


@pytest.mark.parametrize("name", sorted(EXPECTED))
def test_no_string_field_exceeds_64_chars(name):
    payload = _analyze(name).model_dump(mode="json")
    assert _long_strings(payload) == []


@pytest.mark.parametrize("name", sorted(EXPECTED))
def test_no_fixture_text_in_response(name):
    payload = json.dumps(_analyze(name).model_dump(mode="json"))
    for phrase in ("Preamble", "Introduction", "Smith", "quoted", "example.com", "Figure one"):
        assert phrase not in payload


def test_long_string_guard_catches_leaks():
    leak = {"structure": {"sections": [{"label": "x" * 65}]}, "document": {"filename": "y" * 99}}
    assert _long_strings(leak) == [("label", "x" * 65)]


# ── REST endpoints ─────────────────────────────────────────────────────────────


@pytest.fixture
async def client():
    async with httpx.AsyncClient(
        transport=httpx.ASGITransport(app=app), base_url="http://test"
    ) as c:
        yield c


@pytest.mark.parametrize("name", sorted(EXPECTED))
async def test_analyze_upload_endpoint(client, name):
    resp = await client.post(
        "/api/documents/analyze",
        files={"file": (name, (FIXTURES / name).read_bytes(), CONTENT_TYPES[name])},
    )
    assert resp.status_code == 200, resp.text
    body = resp.json()
    assert body["structure"]["totals"] == EXPECTED[name]
    assert body["document"]["filename"] == name
    assert body["meta"]["parser"] == active_parser()
    assert _long_strings(body) == []


async def test_upload_too_large(client, monkeypatch):
    monkeypatch.setattr(settings, "document_max_bytes", 10)
    resp = await client.post(
        "/api/documents/analyze", files={"file": ("a.txt", b"x" * 11, "text/plain")}
    )
    assert resp.status_code == 413
    assert resp.json()["detail"]["error"] == "file_too_large"


async def test_upload_unsupported_type(client):
    resp = await client.post(
        "/api/documents/analyze", files={"file": ("a.png", b"\x89PNG", "image/png")}
    )
    assert resp.status_code == 415
    assert "text/markdown" in resp.json()["detail"]["allowed"]


async def test_upload_malformed_pdf(client):
    resp = await client.post(
        "/api/documents/analyze", files={"file": ("a.pdf", b"%PDF-1.7 junk", "application/pdf")}
    )
    assert resp.status_code == 422
    assert resp.json()["detail"]["error"] == "invalid_document"


async def test_existing_book_routes_untouched(client):
    resp = await client.get("/api/books/structure/schemas")
    assert resp.status_code == 200 and len(resp.json()) == 5


# ── analyze-url ────────────────────────────────────────────────────────────────

DOC_URL = "https://docs.example.com/paper.md"


@respx.mock
async def test_analyze_url(client, public_dns):
    respx.get(DOC_URL).mock(
        return_value=httpx.Response(
            200,
            content=(FIXTURES / "paper.md").read_bytes(),
            headers={"content-type": "text/markdown; charset=utf-8"},
        )
    )
    resp = await client.post("/api/documents/analyze-url", json={"url": DOC_URL})
    assert resp.status_code == 200, resp.text
    body = resp.json()
    assert body["structure"]["totals"] == EXPECTED["paper.md"]
    assert body["document"]["source_url"] == DOC_URL


@respx.mock
async def test_analyze_url_octet_stream_uses_extension(client, public_dns):
    respx.get(DOC_URL).mock(
        return_value=httpx.Response(
            200, content=b"# Hi\n\nText.", headers={"content-type": "application/octet-stream"}
        )
    )
    resp = await client.post("/api/documents/analyze-url", json={"url": DOC_URL})
    assert resp.status_code == 200
    assert resp.json()["document"]["format"] == "markdown"


@pytest.mark.parametrize(
    "addr",
    ["10.0.0.5", "127.0.0.1", "192.168.1.1", "172.16.0.1", "169.254.169.254", "100.64.0.1",
     "0.0.0.0", "::1", "fd00::1", "fe80::1", "::ffff:10.0.0.1", "224.0.0.1"],
)  # fmt: skip
@respx.mock
async def test_analyze_url_blocks_private_addresses(client, public_dns, addr):
    public_dns["internal.example.com"] = [addr]
    route = respx.get(url__startswith="http://internal.example.com")
    resp = await client.post(
        "/api/documents/analyze-url", json={"url": "http://internal.example.com/doc.txt"}
    )
    assert resp.status_code == 400
    assert resp.json()["detail"]["error"] == "url_not_allowed"
    assert not route.called


@pytest.mark.parametrize(
    "url", ["http://127.0.0.1/x.txt", "http://[::1]/x.txt", "http://10.1.2.3/"]
)
@respx.mock
async def test_analyze_url_blocks_literal_ips(client, url):
    resp = await client.post("/api/documents/analyze-url", json={"url": url})
    assert resp.status_code == 400


@respx.mock
async def test_analyze_url_blocks_mixed_dns_answers(client, public_dns):
    public_dns["rebind.example.com"] = ["93.184.216.34", "10.0.0.1"]
    resp = await client.post(
        "/api/documents/analyze-url", json={"url": "https://rebind.example.com/"}
    )
    assert resp.status_code == 400


@respx.mock
async def test_analyze_url_revalidates_redirects(client, public_dns):
    public_dns["metadata.internal"] = ["169.254.169.254"]
    respx.get(DOC_URL).mock(
        return_value=httpx.Response(302, headers={"location": "http://metadata.internal/latest"})
    )
    inner = respx.get("http://metadata.internal/latest")
    resp = await client.post("/api/documents/analyze-url", json={"url": DOC_URL})
    assert resp.status_code == 400
    assert not inner.called


@respx.mock
async def test_analyze_url_follows_public_redirect(client, public_dns):
    respx.get(DOC_URL).mock(return_value=httpx.Response(301, headers={"location": "/v2/paper.txt"}))
    respx.get("https://docs.example.com/v2/paper.txt").mock(
        return_value=httpx.Response(
            200, content=b"Plain text.", headers={"content-type": "text/plain"}
        )
    )
    resp = await client.post("/api/documents/analyze-url", json={"url": DOC_URL})
    assert resp.status_code == 200
    assert resp.json()["structure"]["totals"]["sentences"] == 1


@respx.mock
async def test_analyze_url_redirect_loop(client, public_dns):
    respx.get(DOC_URL).mock(return_value=httpx.Response(302, headers={"location": DOC_URL}))
    resp = await client.post("/api/documents/analyze-url", json={"url": DOC_URL})
    assert resp.status_code == 502


@respx.mock
async def test_analyze_url_timeout(client, public_dns):
    respx.get(DOC_URL).mock(side_effect=httpx.ReadTimeout("slow"))
    resp = await client.post("/api/documents/analyze-url", json={"url": DOC_URL})
    assert resp.status_code == 504
    assert resp.json()["detail"]["error"] == "fetch_timeout"


def test_fetch_timeout_is_ten_seconds():
    assert settings.document_fetch_timeout == 10.0


@respx.mock
async def test_analyze_url_upstream_error(client, public_dns):
    respx.get(DOC_URL).mock(return_value=httpx.Response(404))
    resp = await client.post("/api/documents/analyze-url", json={"url": DOC_URL})
    assert resp.status_code == 502


@respx.mock
async def test_analyze_url_too_large(client, public_dns, monkeypatch):
    monkeypatch.setattr(settings, "document_max_bytes", 100)
    respx.get(DOC_URL).mock(
        return_value=httpx.Response(200, content=b"x" * 101, headers={"content-type": "text/plain"})
    )
    resp = await client.post("/api/documents/analyze-url", json={"url": DOC_URL})
    assert resp.status_code == 413


@respx.mock
async def test_analyze_url_unsupported_type(client, public_dns):
    respx.get(DOC_URL).mock(
        return_value=httpx.Response(200, content=b"\x89PNG", headers={"content-type": "image/png"})
    )
    resp = await client.post("/api/documents/analyze-url", json={"url": DOC_URL})
    assert resp.status_code == 415


async def test_analyze_url_rejects_non_http_scheme(client):
    resp = await client.post("/api/documents/analyze-url", json={"url": "file:///etc/passwd"})
    assert resp.status_code == 422  # pydantic HttpUrl


# ── MCP tool ───────────────────────────────────────────────────────────────────


def _strip_volatile(payload: dict) -> dict:
    meta = {
        k: v for k, v in payload["meta"].items() if k not in ("analyzed_at", "processing_time_ms")
    }
    return {**payload, "meta": meta}


@pytest.mark.parametrize("name", sorted(EXPECTED))
async def test_mcp_matches_rest(client, name):
    data = (FIXTURES / name).read_bytes()
    rest = await client.post(
        "/api/documents/analyze", files={"file": (name, data, CONTENT_TYPES[name])}
    )
    mcp = await mcp_analyze_document(
        document_base64=base64.b64encode(data).decode(),
        content_type=CONTENT_TYPES[name],
        filename=name,
    )
    assert _strip_volatile(mcp) == _strip_volatile(rest.json())


@respx.mock
async def test_mcp_url(public_dns):
    respx.get(DOC_URL).mock(
        return_value=httpx.Response(
            200, content=b"# T\n\nBody.", headers={"content-type": "text/markdown"}
        )
    )
    result = await mcp_analyze_document(url=DOC_URL)
    assert result["structure"]["totals"]["headings"] == 1


async def test_mcp_errors():
    assert "error" in await mcp_analyze_document()
    assert (await mcp_analyze_document(document_base64="!!"))["error"] == "invalid_base64"
    blocked = await mcp_analyze_document(url="http://127.0.0.1/")
    assert (blocked["status"], blocked["error"]) == (400, "url_not_allowed")
    unsupported = await mcp_analyze_document(
        document_base64=base64.b64encode(b"x").decode(), content_type="image/png"
    )
    assert unsupported["status"] == 415


# ── Review fixes (PR #91) ──────────────────────────────────────────────────────


def test_markdown_blockquote_ends_at_unquoted_heading():
    md = b"> Quoted text.\n# Next section\n\nBody text."
    result = analyze_document(md, content_type="text/markdown")
    assert _outline(result.structure.sections) == [("preamble", 0, "0"), ("section", 1, "1")]
    section = result.structure.sections[1]
    assert (section.counts.headings, section.counts.paragraphs) == (1, 1)
    assert result.structure.totals.blockquotes == 1


def test_markdown_lazy_quote_continuation_still_joins_paragraph_text():
    totals = analyze_document(
        b"> Quoted start\ncontinues lazily.", content_type="text/markdown"
    ).structure.totals
    assert (totals.blockquotes, totals.paragraphs, totals.sentences) == (1, 1, 1)


def test_markdown_indented_list_marker_is_code():
    totals = analyze_document(
        b"    - not a list\n    - still code", content_type="text/markdown"
    ).structure.totals
    assert (totals.code_blocks, totals.lists, totals.list_items, totals.words) == (1, 0, 0, 0)


@pytest.mark.parametrize(
    ("ctype", "body"),
    [
        ("text/markdown", b"# [Title](https://example.org)\n\nText."),
        ("text/markdown", b"[Title](https://example.org)\n===\n\nText."),
        ("text/plain", b"1. See https://example.org/spec\n\nText."),
    ],
)
def test_heading_links_are_counted(ctype, body):
    totals = analyze_document(body, content_type=ctype).structure.totals
    assert (totals.headings, totals.links) == (1, 1)


@respx.mock
async def test_analyze_url_uses_final_url_for_extension(client, public_dns):
    start = "https://docs.example.com/latest"
    respx.get(start).mock(
        return_value=httpx.Response(302, headers={"location": "/files/paper.pdf"})
    )
    respx.get("https://docs.example.com/files/paper.pdf").mock(
        return_value=httpx.Response(
            200,
            content=(FIXTURES / "paper.pdf").read_bytes(),
            headers={"content-type": "application/octet-stream"},
        )
    )
    resp = await client.post("/api/documents/analyze-url", json={"url": start})
    assert resp.status_code == 200, resp.text
    body = resp.json()
    assert body["document"]["format"] == "pdf"
    assert body["document"]["source_url"] == start
    assert body["structure"]["totals"] == EXPECTED["paper.pdf"]
