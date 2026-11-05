"""Request-scoped anonymous browser identity, never a global default user."""

from contextvars import ContextVar

browser_user_id: ContextVar[str | None] = ContextVar("browser_user_id", default=None)
