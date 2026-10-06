"""
Text fetcher service.

Responsibilities
----------------
- Fetch raw book text from a Project Gutenberg URL (plain-text or HTML).
- Strip HTML tags when the source is HTML.
- Remove Project Gutenberg boilerplate (header / footer between *** markers).
- Remove confirmed opening contents pages before structural counting.
- Normalise whitespace.
- Cache the cleaned text by gutenberg_id (TTL driven by settings).
- Enforce a rate limit on outbound Gutenberg fetches.
"""

import asyncio
import re
import time

import httpx

from metabook_py.core.cache import book_text_cache
from metabook_py.core.config import settings
from metabook_py.core.exceptions import TextUnavailableError
from metabook_py.services.preprocessing import preprocess_text, strip_boilerplate

# ── Rate-limit state (module-level, shared within the process) ─────────────────
_fetch_lock = asyncio.Lock()
_last_fetch_at: float = 0.0


# ── Regex constants ────────────────────────────────────────────────────────────
_HTML_BLOCK = re.compile(r"</(?:p|div|h[1-6]|section|article|pre)>|<br\s*/?>", re.IGNORECASE)
_HTML_TAG = re.compile(r"<[^>]+>")
_HTML_ENTITY = re.compile(r"&(?:#\d+|#x[\da-fA-F]+|[a-zA-Z]+);")


# ── Private helpers ────────────────────────────────────────────────────────────


def _strip_html(text: str) -> str:
    text = _HTML_BLOCK.sub("\n\n", text)
    text = _HTML_TAG.sub(" ", text)
    text = _HTML_ENTITY.sub(" ", text)
    return "\n".join(line.strip() for line in text.splitlines())


def _strip_boilerplate(text: str) -> str:
    """Compatibility wrapper for the shared Gutenberg boundary cleanup."""
    return strip_boilerplate(text)


def _normalise(raw: str, *, is_html: bool) -> str:
    if is_html:
        raw = _strip_html(raw)

    return preprocess_text(raw)


async def _rate_limited_get(url: str) -> str:
    """Fetch URL text, honouring the configured rate limit."""
    global _last_fetch_at

    async with _fetch_lock:
        gap = 1.0 / settings.rate_limit_per_second
        elapsed = time.monotonic() - _last_fetch_at
        if elapsed < gap:
            await asyncio.sleep(gap - elapsed)

        async with httpx.AsyncClient(timeout=60.0, follow_redirects=True) as client:
            resp = await client.get(url)
            resp.raise_for_status()

        _last_fetch_at = time.monotonic()

        # Attempt UTF-8 first, fall back to latin-1 (covers most Gutenberg texts)
        try:
            return resp.content.decode("utf-8")
        except UnicodeDecodeError:
            return resp.content.decode("latin-1", errors="replace")


# ── Public API ─────────────────────────────────────────────────────────────────


async def fetch_book_text(
    gutenberg_id: int,
    download_url: str,
    *,
    is_html: bool = False,
) -> tuple[str, bool]:
    """
    Return (cleaned_text, was_cached).

    The cleaned text has:
    - HTML tags stripped (if is_html)
    - Project Gutenberg boilerplate and confirmed opening contents pages removed
    - Windows line-endings normalised
    - Runs of 3+ blank lines collapsed to 2

    Raises
    ------
    TextUnavailableError — fetch failure OR empty text after cleaning
    """
    cache_key = f"book_text:{gutenberg_id}"

    cached = await book_text_cache.get(cache_key)
    if cached is not None:
        return cached, True

    try:
        raw = await _rate_limited_get(download_url)
    except (httpx.HTTPError, httpx.TimeoutException) as exc:
        raise TextUnavailableError(gutenberg_id=gutenberg_id) from exc

    text = _normalise(raw, is_html=is_html)

    if not text:
        raise TextUnavailableError(gutenberg_id=gutenberg_id)

    await book_text_cache.set(cache_key, text)
    return text, False
