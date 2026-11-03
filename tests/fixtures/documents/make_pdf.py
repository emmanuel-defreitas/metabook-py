"""
Generate paper.pdf — a 2-page fixture with known layout (no dependencies).

    uv run python tests/fixtures/documents/make_pdf.py

Body text is 11 pt Helvetica on a 14 pt line pitch; paragraphs are separated
by a 28 pt gap. Headings: 20 pt (level 1), 16 pt (level 2), 13 pt (level 3).
"""

from __future__ import annotations

from pathlib import Path

# (font size, y position, text) per page; x is always 72.
PAGES: list[list[tuple[int, int, str]]] = [
    [
        (11, 740, "Preamble paragraph before any heading. It continues"),
        (11, 726, "on this line."),
        (20, 690, "Document Structure"),
        (11, 664, "Opening paragraph of the paper. It has two sentences."),
        (16, 630, "Introduction"),
        (11, 606, "Dr. Smith wrote the first draft. The second sen-"),
        (11, 592, "tence follows! Does a third one end here?"),
        (11, 564, "- First point"),
        (11, 550, "- Second point"),
        (11, 536, "- Third point"),
        (13, 504, "Background"),
        (11, 480, "Figure 1. A caption for the first figure."),
        (9, 60, "1"),
    ],
    [
        (16, 740, "Methods"),
        (11, 716, "A paragraph on the second page. It has two sentences."),
        (16, 680, "References"),
        (11, 656, "[1] Smith, J. Structure of things. 2020."),
        (11, 628, "[2] Doe, A. Counting words. 2021."),
        (9, 60, "2"),
    ],
]


def _escape(text: str) -> str:
    return text.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")


def build_pdf(pages: list[list[tuple[int, int, str]]]) -> bytes:
    objects: list[bytes] = []

    def add(body: bytes) -> int:
        objects.append(body)
        return len(objects)

    font = add(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>")
    pages_id = len(objects) + 1 + 2 * len(pages)  # reserved: after page/content pairs
    page_ids: list[int] = []
    for lines in pages:
        stream = "\n".join(
            f"BT /F1 {size} Tf 72 {y} Td ({_escape(text)}) Tj ET" for size, y, text in lines
        ).encode("latin-1")
        content = add(b"<< /Length %d >>\nstream\n" % len(stream) + stream + b"\nendstream")
        page_ids.append(
            add(
                b"<< /Type /Page /Parent %d 0 R /MediaBox [0 0 612 792] "
                b"/Resources << /Font << /F1 %d 0 R >> >> /Contents %d 0 R >>"
                % (pages_id, font, content)
            )
        )
    kids = b" ".join(b"%d 0 R" % p for p in page_ids)
    assert add(b"<< /Type /Pages /Kids [%s] /Count %d >>" % (kids, len(page_ids))) == pages_id
    catalog = add(b"<< /Type /Catalog /Pages %d 0 R >>" % pages_id)

    out = bytearray(b"%PDF-1.4\n")
    offsets = []
    for i, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out += b"%d 0 obj\n" % i + body + b"\nendobj\n"
    xref = len(out)
    out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objects) + 1)
    out += b"".join(b"%010d 00000 n \n" % off for off in offsets)
    out += b"trailer\n<< /Size %d /Root %d 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (
        len(objects) + 1,
        catalog,
        xref,
    )
    return bytes(out)


if __name__ == "__main__":
    target = Path(__file__).with_name("paper.pdf")
    target.write_bytes(build_pdf(PAGES))
    print(f"wrote {target} ({target.stat().st_size} bytes)")
