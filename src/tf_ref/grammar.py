from __future__ import annotations

import re
from urllib.parse import quote, unquote

from .errors import ParseError
from .reference import Reference

_SELECTOR = re.compile(r"([A-Za-z0-9_]+)([1-9][0-9]*)(?:-([1-9][0-9]*))?\Z")
_URN_SAFE = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~"


def escape_value(value: str) -> str:
    if any(char in value for char in ':/!@"') or any(char.isspace() for char in value):
        return f'"{value.replace(chr(34), chr(34) * 2)}"'
    if not value:
        return '""'
    return value


def _split_quoted(text: str, delimiter: str) -> list[str]:
    parts: list[str] = []
    start = 0
    quoted = False
    index = 0
    while index < len(text):
        char = text[index]
        if char == '"':
            if quoted and index + 1 < len(text) and text[index + 1] == '"':
                index += 2
                continue
            quoted = not quoted
        elif char == delimiter and not quoted:
            parts.append(text[start:index])
            start = index + 1
        index += 1
    if quoted:
        raise ParseError(text, "unterminated quoted section")
    parts.append(text[start:])
    return parts


def _unescape_value(value: str, whole: str) -> str:
    if value.startswith('"'):
        if len(value) < 2 or not value.endswith('"'):
            raise ParseError(whole, "unterminated quoted section")
        inner = value[1:-1]
        index = 0
        decoded: list[str] = []
        while index < len(inner):
            if inner[index] == '"':
                if index + 1 >= len(inner) or inner[index + 1] != '"':
                    raise ParseError(whole, "a quote inside a section must be doubled")
                decoded.append('"')
                index += 2
            else:
                decoded.append(inner[index])
                index += 1
        return "".join(decoded)
    if not value or any(char in value for char in ':/!@"') or any(char.isspace() for char in value):
        raise ParseError(whole, "invalid unquoted section")
    return value


def _identity(text: str, whole: str) -> tuple[str, str | None]:
    pieces = text.split("@")
    if len(pieces) > 2 or not pieces[0] or (len(pieces) == 2 and not pieces[1]):
        raise ParseError(whole, "invalid corpus identity")
    return pieces[0], pieces[1] if len(pieces) == 2 else None


def _selector(text: str, whole: str) -> tuple[str | None, int | None, int | None]:
    if not text:
        return None, None, None
    match = _SELECTOR.fullmatch(text)
    if match is None:
        raise ParseError(whole, f"invalid selector {text!r}")
    start = int(match[2])
    end = int(match[3]) if match[3] else None
    if end is not None and end < start:
        raise ParseError(whole, f"descending range {text!r}")
    return match[1], start, end


def _check_depth(sections: tuple[str, ...], depth: int | None, whole: str) -> None:
    if depth is not None and len(sections) > depth:
        raise ParseError(sections[depth], f"reference exceeds corpus depth {depth}: {whole!r}")


def _parse_short(text: str, depth: int | None) -> Reference:
    selector_parts = _split_quoted(text, "!")
    if len(selector_parts) > 2:
        raise ParseError(text, "more than one selector delimiter")
    if len(selector_parts) == 2 and not selector_parts[1]:
        raise ParseError(text, "selector is empty")
    body, selector_text = selector_parts[0], selector_parts[1] if len(selector_parts) == 2 else ""
    prefix_parts = _split_quoted(body, "/")
    if len(prefix_parts) > 2:
        raise ParseError(text, "more than one corpus delimiter")
    corpus_id = version = None
    sections_text = prefix_parts[-1]
    if len(prefix_parts) == 2:
        corpus_id, version = _identity(prefix_parts[0], text)
    raw_sections = _split_quoted(sections_text, ":") if sections_text else []
    if not raw_sections:
        raise ParseError(text, "at least one section is required")
    sections = tuple(_unescape_value(value, text) for value in raw_sections)
    _check_depth(sections, depth, text)
    otype, start, end = _selector(selector_text, text)
    return Reference(corpus_id, version, sections, otype, start, end)


def _parse_urn(text: str, depth: int | None) -> Reference:
    body = text[len("urn:tf:") :]
    selector_parts = body.split("!")
    if len(selector_parts) > 2:
        raise ParseError(text, "more than one selector delimiter")
    if len(selector_parts) == 2 and not selector_parts[1]:
        raise ParseError(text, "selector is empty")
    main, selector_text = selector_parts[0], selector_parts[1] if len(selector_parts) == 2 else ""
    identity, separator, sections_text = main.partition(":")
    if not separator or not sections_text:
        raise ParseError(text, "URN requires corpus identity and sections")
    corpus_id, version = _identity(identity, text)
    raw_sections = sections_text.split(":")
    for value in raw_sections:
        if re.search(r"%(?![0-9A-Fa-f]{2})", value):
            raise ParseError(text, f"invalid percent escape {value!r}")
    sections = tuple(unquote(value) for value in raw_sections)
    _check_depth(sections, depth, text)
    otype, start, end = _selector(selector_text, text)
    return Reference(corpus_id, version, sections, otype, start, end)


def parse(text: str, *, depth: int | None = None) -> Reference:
    if not text:
        raise ParseError(text, "reference is empty")
    return _parse_urn(text, depth) if text.startswith("urn:tf:") else _parse_short(text, depth)


def format_ref(ref: Reference, *, urn: bool = False) -> str:
    selector = ""
    if ref.otype is not None:
        selector = f"!{ref.otype}{ref.start}"
        if ref.end is not None:
            selector += f"-{ref.end}"
    identity = ""
    if ref.corpus_id is not None:
        identity = ref.corpus_id + (f"@{ref.version}" if ref.version is not None else "")
    if urn:
        if not identity:
            raise ParseError("", "URN output requires a corpus id")
        sections = ":".join(quote(value, safe=_URN_SAFE) for value in ref.sections)
        return f"urn:tf:{identity}:{sections}{selector}"
    sections = ":".join(escape_value(value) for value in ref.sections)
    return f"{identity + '/' if identity else ''}{sections}{selector}"
