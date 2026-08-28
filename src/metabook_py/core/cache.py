"""
Lightweight async in-memory cache.

Design notes
------------
- Keyed by string, values can be any picklable object.
- TTL enforced on read (lazy eviction) — good enough for a book-text cache
  where keys are bounded by the number of unique Gutenberg IDs requested.
- Thread-safety via asyncio.Lock (single-event-loop assumption, fine for FastAPI).
- To swap for Redis: replace AsyncInMemoryCache with an aioredis wrapper that
  exposes the same get / set / delete / clear interface.
"""

import asyncio
import time
from typing import Any

from metabook_py.core.config import settings


class AsyncInMemoryCache:
    def __init__(self, default_ttl: int = settings.cache_ttl):
        self._store: dict[str, tuple[Any, float]] = {}
        self._default_ttl = default_ttl
        self._lock = asyncio.Lock()

    async def get(self, key: str) -> Any | None:
        async with self._lock:
            entry = self._store.get(key)
            if entry is None:
                return None
            value, expires_at = entry
            if time.monotonic() < expires_at:
                return value
            # Expired — evict lazily
            del self._store[key]
            return None

    async def set(self, key: str, value: Any, ttl: int | None = None) -> None:
        ttl = ttl if ttl is not None else self._default_ttl
        async with self._lock:
            self._store[key] = (value, time.monotonic() + ttl)

    async def delete(self, key: str) -> None:
        async with self._lock:
            self._store.pop(key, None)

    def clear(self) -> None:
        """Synchronous clear — safe to call at shutdown."""
        self._store.clear()

    @property
    def size(self) -> int:
        return len(self._store)


# Module-level singleton — shared across all requests in the process
book_text_cache = AsyncInMemoryCache()
