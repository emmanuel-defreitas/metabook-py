"""
EPUB parsing service.

Responsibilities
----------------
- Validate that uploaded bytes are a real EPUB (zip container + mimetype).
- Resolve the OPF package document via META-INF/container.xml.
- Extract Dublin Core metadata (title, creators, language, subjects, ISBN).
- Walk the spine in reading order, extract plain text from every XHTML
  document (headings on their own lines so the schema detector sees them).
- Return (metadata, cleaned_text) ready for detect_schema / counter.

Pure stdlib — zipfile + xml.etree + html.parser. No text content is
persisted; the caller decides what to do with it.
"""

from __future__ import annotations

import posixpath
import re
import zipfile
from dataclasses import dataclass, field
from html.parser import HTMLParser
from io import BytesIO
from xml.etree import ElementTree

from metabook_py.core.exceptions import InvalidEpubError

# ── XML namespaces ─────────────────────────────────────────────────────────────

_NS = {
    "container": "urn:oasis:names:tc:opendocument:xmlns:container",
    "opf": "http://www.idpf.org/2007/opf",
    "dc": "http://purl.org/dc/elements/1.1/",
}

_ISBN_RE = re.compile(r"(?:97[89][-\s]?)?(?:\d[-\s]?){9}[\dXx]")
_MULTI_BLANK = re.compile(r"\n{3,}")


# ── Result types ───────────────────────────────────────────────────────────────


@dataclass
class EpubMetadata:
    title: str = "Untitled"
    authors: list[str] = field(default_factory=list)
    language: str = "en"
    subjects: list[str] = field(default_factory=list)
    isbn: str | None = None


@dataclass
class ParsedEpub:
    metadata: EpubMetadata
    text: str
    spine_document_count: int


# ── XHTML → text extraction ────────────────────────────────────────────────────

_BLOCK_TAGS = frozenset(
    {
        "p",
        "div",
        "section",
        "article",
        "blockquote",
        "li",
        "tr",
        "td",
        "th",
        "figcaption",
        "pre",
        "br",
        "hr",
    }
)
_HEADING_TAGS = frozenset({"h1", "h2", "h3", "h4", "h5", "h6"})
_SKIP_TAGS = frozenset({"script", "style", "head", "title"})


class _TextExtractor(HTMLParser):
    """Collect visible text; block elements and headings become paragraph breaks."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self._chunks: list[str] = []
        self._skip_depth = 0

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag in _SKIP_TAGS:
            self._skip_depth += 1
        elif tag in _HEADING_TAGS or tag in _BLOCK_TAGS:
            self._chunks.append("\n\n")

    def handle_endtag(self, tag: str) -> None:
        if tag in _SKIP_TAGS and self._skip_depth > 0:
            self._skip_depth -= 1
        elif tag in _HEADING_TAGS or tag in _BLOCK_TAGS:
            self._chunks.append("\n\n")

    def handle_data(self, data: str) -> None:
        if self._skip_depth == 0 and data.strip():
            self._chunks.append(data)

    def text(self) -> str:
        raw = "".join(self._chunks)
        # Collapse intra-paragraph whitespace but preserve the \n\n breaks
        paragraphs = [re.sub(r"\s+", " ", p).strip() for p in raw.split("\n\n")]
        return "\n\n".join(p for p in paragraphs if p)


def _xhtml_to_text(markup: str) -> str:
    extractor = _TextExtractor()
    extractor.feed(markup)
    return extractor.text()


# ── OPF helpers ────────────────────────────────────────────────────────────────


def _find_opf_path(zf: zipfile.ZipFile) -> str:
    try:
        container = ElementTree.fromstring(zf.read("META-INF/container.xml"))
    except (KeyError, ElementTree.ParseError) as exc:
        raise InvalidEpubError("META-INF/container.xml is missing or malformed") from exc

    rootfile = container.find(".//container:rootfile", _NS)
    opf_path = rootfile.get("full-path") if rootfile is not None else None
    if not opf_path:
        raise InvalidEpubError("container.xml does not declare a rootfile")
    return opf_path


def _parse_metadata(opf: ElementTree.Element) -> EpubMetadata:
    meta = EpubMetadata()

    title = opf.find(".//dc:title", _NS)
    if title is not None and title.text and title.text.strip():
        meta.title = title.text.strip()

    meta.authors = [
        el.text.strip() for el in opf.findall(".//dc:creator", _NS) if el.text and el.text.strip()
    ]
    meta.subjects = [
        el.text.strip() for el in opf.findall(".//dc:subject", _NS) if el.text and el.text.strip()
    ]

    lang = opf.find(".//dc:language", _NS)
    if lang is not None and lang.text and lang.text.strip():
        meta.language = lang.text.strip().split("-")[0].lower()

    for ident in opf.findall(".//dc:identifier", _NS):
        candidate = (ident.text or "").strip()
        match = _ISBN_RE.search(candidate)
        if match:
            meta.isbn = re.sub(r"[-\s]", "", match.group(0))
            break

    return meta


def _spine_hrefs(opf: ElementTree.Element) -> list[str]:
    """Return manifest hrefs referenced by the spine, in reading order."""
    manifest: dict[str, tuple[str, str]] = {}  # id → (href, media-type)
    for item in opf.findall(".//opf:manifest/opf:item", _NS):
        item_id, href = item.get("id"), item.get("href")
        if item_id and href:
            manifest[item_id] = (href, item.get("media-type", ""))

    hrefs: list[str] = []
    for itemref in opf.findall(".//opf:spine/opf:itemref", _NS):
        entry = manifest.get(itemref.get("idref", ""))
        if entry and ("html" in entry[1] or entry[0].endswith((".xhtml", ".html", ".htm"))):
            hrefs.append(entry[0])
    return hrefs


# ── Public API ─────────────────────────────────────────────────────────────────


def parse_epub(data: bytes) -> ParsedEpub:
    """
    Parse EPUB bytes into (metadata, plain text).

    Raises
    ------
    InvalidEpubError — not a zip, no container/OPF, or no extractable text.
    """
    try:
        zf = zipfile.ZipFile(BytesIO(data))
    except zipfile.BadZipFile as exc:
        raise InvalidEpubError("File is not a valid EPUB (zip) archive") from exc

    with zf:
        # The mimetype entry is required by spec but some producers omit it;
        # only reject when it exists and is wrong.
        if "mimetype" in zf.namelist():
            declared = zf.read("mimetype").decode("ascii", errors="replace").strip()
            if declared != "application/epub+zip":
                raise InvalidEpubError(f"Unexpected mimetype: {declared!r}")

        opf_path = _find_opf_path(zf)
        try:
            opf = ElementTree.fromstring(zf.read(opf_path))
        except (KeyError, ElementTree.ParseError) as exc:
            raise InvalidEpubError(
                f"OPF package document {opf_path!r} is missing or malformed"
            ) from exc

        metadata = _parse_metadata(opf)
        opf_dir = posixpath.dirname(opf_path)

        texts: list[str] = []
        spine_count = 0
        for href in _spine_hrefs(opf):
            path = posixpath.normpath(posixpath.join(opf_dir, href) if opf_dir else href)
            try:
                markup = zf.read(path).decode("utf-8", errors="replace")
            except KeyError:
                continue  # spine points at a missing file — skip, don't fail
            doc_text = _xhtml_to_text(markup)
            if doc_text:
                texts.append(doc_text)
                spine_count += 1

    text = _MULTI_BLANK.sub("\n\n", "\n\n".join(texts)).strip()
    if not text:
        raise InvalidEpubError("No extractable text found in the EPUB spine")

    return ParsedEpub(metadata=metadata, text=text, spine_document_count=spine_count)
