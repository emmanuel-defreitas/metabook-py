"""
SSRF-protected URL fetcher (single-document analysis, MCP EPUB downloads).

- http/https only; no credentials in the URL.
- Every address the host resolves to must be globally routable and not
  multicast (blocks private, loopback, link-local, CGNAT, reserved, …).
- Redirects are followed manually so each hop is re-validated (max 5).
- 10 s total budget; body streamed and capped at the document size limit.

Known limit: the address is checked before httpx connects, so a DNS answer
that changes between the check and the connect (rebinding) is not caught.
"""

from __future__ import annotations

import asyncio
import ipaddress
import socket
from urllib.parse import urljoin, urlsplit

import httpx

from metabook_py.core.config import settings
from metabook_py.core.exceptions import (
    DocumentFetchError,
    DocumentTooLargeError,
    UrlNotAllowedError,
)

_REDIRECTS = {301, 302, 303, 307, 308}


async def resolve_host(host: str) -> list[str]:
    """Resolve *host* to IP strings (patched in tests)."""
    loop = asyncio.get_running_loop()
    infos = await loop.getaddrinfo(host, None, type=socket.SOCK_STREAM)
    return sorted({str(info[4][0]) for info in infos})


def _is_public(addr: str) -> bool:
    ip = ipaddress.ip_address(addr.split("%", 1)[0])
    if isinstance(ip, ipaddress.IPv6Address) and ip.ipv4_mapped is not None:
        ip = ip.ipv4_mapped
    return ip.is_global and not ip.is_multicast


async def validate_url(url: str) -> None:
    parts = urlsplit(url)
    if parts.scheme not in ("http", "https"):
        raise UrlNotAllowedError("only http and https URLs are allowed")
    if parts.username or parts.password:
        raise UrlNotAllowedError("credentials in URLs are not allowed")
    host = parts.hostname
    if not host:
        raise UrlNotAllowedError("URL has no host")
    try:
        addrs = [host] if _is_ip(host) else await resolve_host(host)
    except OSError as exc:
        raise DocumentFetchError(f"could not resolve host: {exc}") from exc
    if not addrs:
        raise DocumentFetchError("host resolved to no addresses")
    if not all(_is_public(a) for a in addrs):
        raise UrlNotAllowedError("host resolves to a non-public address")


def _is_ip(host: str) -> bool:
    try:
        ipaddress.ip_address(host)
    except ValueError:
        return False
    return True


async def _fetch(url: str, limit: int, timeout: float) -> tuple[bytes, str | None, str]:
    async with httpx.AsyncClient(follow_redirects=False, timeout=httpx.Timeout(timeout)) as client:
        for _ in range(settings.document_max_redirects + 1):
            await validate_url(url)
            async with client.stream("GET", url) as resp:
                if resp.status_code in _REDIRECTS and "location" in resp.headers:
                    url = urljoin(url, resp.headers["location"])
                    continue
                if resp.status_code >= 400:
                    raise DocumentFetchError(f"upstream returned HTTP {resp.status_code}")
                declared = resp.headers.get("content-length", "")
                if declared.isdigit() and int(declared) > limit:
                    raise DocumentTooLargeError(limit)
                body = bytearray()
                async for chunk in resp.aiter_bytes():
                    body.extend(chunk)
                    if len(body) > limit:
                        raise DocumentTooLargeError(limit)
                return bytes(body), resp.headers.get("content-type"), url
    raise DocumentFetchError("too many redirects")


async def fetch_url(url: str, *, max_bytes: int, timeout: float) -> tuple[bytes, str | None, str]:
    """SSRF-checked GET of a public http(s) URL. Returns (body, content type, final URL).

    Raises UrlNotAllowedError, DocumentTooLargeError (body over *max_bytes*),
    and DocumentFetchError (upstream error, too many redirects, or *timeout*
    seconds exceeded for the whole fetch).
    """
    try:
        return await asyncio.wait_for(_fetch(url, max_bytes, timeout), timeout=timeout)
    except TimeoutError as exc:
        raise DocumentFetchError("fetch timed out", timed_out=True) from exc
    except httpx.TimeoutException as exc:
        raise DocumentFetchError("fetch timed out", timed_out=True) from exc
    except httpx.HTTPError as exc:
        raise DocumentFetchError(str(exc) or type(exc).__name__) from exc


async def fetch_document(url: str) -> tuple[bytes, str | None, str]:
    """fetch_url with the single-document limits (20 MB, 10 s)."""
    return await fetch_url(
        url, max_bytes=settings.document_max_bytes, timeout=settings.document_fetch_timeout
    )
