"""
Vercel Blob storage service.

Uploads files via the Vercel Blob REST API:
    PUT https://blob.vercel-storage.com/<pathname>
    Authorization: Bearer <BLOB_READ_WRITE_TOKEN>

Files land under the folder configured by settings.blob_folder ("books").
A random suffix is requested so identical filenames never collide.
"""

from __future__ import annotations

import re
from dataclasses import dataclass

import httpx

from metabook_py.core.config import settings
from metabook_py.core.exceptions import BlobUploadError

_UNSAFE_CHARS = re.compile(r"[^a-zA-Z0-9._-]+")
_BLOB_API_VERSION = "7"


@dataclass
class BlobResult:
    url: str
    pathname: str
    size: int


def sanitize_filename(filename: str) -> str:
    """Keep only the basename, replace unsafe characters with '-'."""
    base = filename.rsplit("/", 1)[-1].rsplit("\\", 1)[-1]
    safe = _UNSAFE_CHARS.sub("-", base).strip("-.")
    return safe or "book.epub"


async def upload_epub(filename: str, content: bytes) -> BlobResult:
    """
    Upload EPUB bytes to Vercel Blob under `<blob_folder>/<filename>`.

    Raises
    ------
    BlobUploadError — token missing, network failure, or non-2xx response.
    """
    if not settings.blob_read_write_token:
        raise BlobUploadError("BLOB_READ_WRITE_TOKEN is not configured")

    pathname = f"{settings.blob_folder}/{sanitize_filename(filename)}"
    url = f"{settings.blob_api_url.rstrip('/')}/{pathname}"

    try:
        async with httpx.AsyncClient(timeout=120.0) as client:
            resp = await client.put(
                url,
                content=content,
                headers={
                    "Authorization": f"Bearer {settings.blob_read_write_token}",
                    "x-api-version": _BLOB_API_VERSION,
                    "x-vercel-blob-access": settings.blob_access,
                    "x-content-type": "application/epub+zip",
                    "x-add-random-suffix": "1",
                },
            )
            resp.raise_for_status()
    except httpx.HTTPStatusError as exc:
        raise BlobUploadError(
            f"Blob store rejected the upload ({exc.response.status_code})"
        ) from exc
    except httpx.HTTPError as exc:
        raise BlobUploadError(f"Blob upload failed: {exc}") from exc

    payload = resp.json()
    return BlobResult(
        url=payload.get("url", ""),
        pathname=payload.get("pathname", pathname),
        size=len(content),
    )
