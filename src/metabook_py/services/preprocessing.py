"""Conservative shared cleanup before detection and counting.

Indices in the resulting tree are ordinals within the cleaned body, not
character offsets into the download. Cleanup is idempotent.
"""

import re

_PG_START = re.compile(
    r"^[ \t]*\*{3}[ \t]*START OF (?:(?:THE|THIS)[ \t]+)?PROJECT GUTENBERG[^*]{0,300}\*{3}[ \t]*$",
    re.IGNORECASE | re.MULTILINE,
)
_PG_END = re.compile(
    r"^[ \t]*(?:\*{3}[ \t]*END OF (?:(?:THE|THIS)[ \t]+)?PROJECT GUTENBERG[^*]{0,300}\*{3}|"
    r"End of (?:the )?Project Gutenberg(?:'s)? (?:EBook|Etext)[^\n]*)[ \t]*$",
    re.IGNORECASE | re.MULTILINE,
)
_CONTENTS = re.compile(r"^(?:TABLE OF )?CONTENTS[.:]?$", re.IGNORECASE)
_ENTRY = re.compile(
    r"^(?:(?:CHAPTER|CHAP\.?|PART|BOOK|SECTION|VOLUME)\s+\S+[^\n]*|"
    r"[^\n]+(?:\.{2,}|\s{2,})\s*\d+|[IVXLC\d]+\.?(?:\s+[^\n]+)?|"
    r"[A-Z][A-Z '\"—-]{3,80})$",
    re.IGNORECASE,
)


def strip_boilerplate(text: str) -> str:
    """Recognise explicit Gutenberg boundaries; never guess a missing header."""
    start = _PG_START.search(text)
    if start:
        text = text[start.end() :]
    end = _PG_END.search(text)
    if end:
        text = text[: end.start()]
    return text


def _is_entry(block: str) -> bool:
    lines = block.splitlines()
    return bool(lines) and all(
        len(line) <= 100
        and len(line.split()) <= 12
        and not re.search(r"[!?]|[a-z]\.(?:\s|$)", line)
        and _ENTRY.fullmatch(line.strip())
        for line in lines
    )


def _entry_key(line: str) -> str:
    """Compare body headings with earlier contents entries without page numbers."""
    line = re.sub(r"(?:\.{2,}|\s{2,})\s*\d+$", "", line).strip().casefold()
    numbered = re.match(r"(?:chapter|chap\.?|part|book|section|volume)\s+\w+", line)
    return numbered[0] if numbered else line


def _strip_contents(text: str) -> str:
    blocks = text.split("\n\n")
    # Only consider an explicitly labelled opening contents page, preceded
    # by short title/author metadata. Never remove a preceding prose chapter.
    for i, block in enumerate(blocks[:10]):
        if _CONTENTS.fullmatch(block.strip()):
            prefix = blocks[:i]
            if any(len(p.split()) > 12 or re.search(r"[.!?]", p) for p in prefix):
                return text
            entries = 0
            seen: set[str] = set()
            for j in range(i + 1, len(blocks)):
                current = blocks[j].strip()
                # A heading followed by prose is the beginning of the body,
                # even when it repeats an entry in the table of contents.
                following = blocks[j + 1].strip() if j + 1 < len(blocks) else ""
                if (
                    len(current.splitlines()) == 1
                    and _is_entry(current)
                    and not re.search(r"(?:\.{2,}|\s{2,})\s*\d+$", current)
                    and following
                    and not _is_entry(following)
                    and entries >= 2
                    and _entry_key(current) in seen
                ):
                    return "\n\n".join(blocks[j:])
                if not _is_entry(current):
                    return "\n\n".join(blocks[j:]) if entries >= 2 else text
                entries += len(current.splitlines())
                seen.update(_entry_key(line) for line in current.splitlines())
            return text  # A contents-only input has no confirmed body.
    return text


def preprocess_text(text: str) -> str:
    """Strip explicit license boundaries and confirmed opening contents pages.

    Unmarked prefaces, dedications and title pages are retained when their
    boundary is uncertain; keeping real prose takes precedence over cleanup.
    """
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    text = strip_boilerplate(text)
    text = re.sub(r"\n[ \t]+\n", "\n\n", text)
    text = re.sub(r"\n{3,}", "\n\n", text).strip()
    return _strip_contents(text)
