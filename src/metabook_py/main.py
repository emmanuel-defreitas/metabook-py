"""
Application entry point.

Wires together:
  - FastAPI REST API  (routers/books.py)
  - FastMCP HTTP server (mcp_server.py) mounted at /mcp

Run locally:
    uv run uvicorn metabook_py.main:app --reload

Run in Docker:
    docker compose up
"""

import warnings
from contextlib import asynccontextmanager

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse

from metabook_py.core.config import settings
from metabook_py.core.version import package_version
from metabook_py.routers.books import router as books_router
from metabook_py.services.postgres import PersistenceError
from metabook_py.services.store import get_upload_store, reset_upload_store

# ── Lifespan ───────────────────────────────────────────────────────────────────


@asynccontextmanager
async def lifespan(app: FastAPI):  # noqa: ANN201
    store = get_upload_store()
    await store.initialize()
    try:
        yield
    finally:
        await store.close()
        reset_upload_store()
    # Graceful shutdown: flush in-memory cache so tests don't bleed state
    from metabook_py.core.cache import book_text_cache

    book_text_cache.clear()


# ── App factory ────────────────────────────────────────────────────────────────

app = FastAPI(
    title=settings.app_name,
    version=package_version(),
    description=(
        "Analyses the structural metadata of Project Gutenberg books. "
        "Returns counts of chapters, paragraphs, sentences, and words per node. "
        "No book text is included in any response."
    ),
    docs_url="/api/docs",
    redoc_url="/api/redoc",
    lifespan=lifespan,
)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_methods=["GET", "POST", "OPTIONS"],
    allow_headers=["*"],
)

# ── Routes ─────────────────────────────────────────────────────────────────────

app.include_router(books_router, prefix="/api")


@app.exception_handler(PersistenceError)
async def persistence_error_handler(request, exc: PersistenceError):
    return JSONResponse(
        status_code=503,
        content={"detail": {"error": "database_unavailable", "message": str(exc)}},
    )


@app.get("/health", tags=["meta"])
async def health() -> dict:
    """Readiness probe, including configured PostgreSQL connectivity."""
    from metabook_py.core.cache import book_text_cache

    if settings.database_url:
        await get_upload_store().check_health()
    return {
        "status": "ok",
        "version": app.version,
        "cache_entries": book_text_cache.size,
        "persistence": "postgresql"
        if settings.database_url
        else ("mongodb" if settings.mongodb_uri else "disabled"),
    }


# ── Mount FastMCP (HTTP / SSE transport) ───────────────────────────────────────
# FastMCP 2.x exposes http_app() which returns a Starlette sub-application.
# If the installed version uses a different method name, adjust here.

try:
    from metabook_py.mcp_server import mcp

    mcp_asgi = mcp.http_app(path="/")
    app.mount("/mcp", mcp_asgi)
except Exception as _mcp_err:  # pragma: no cover
    warnings.warn(
        f"FastMCP HTTP mount failed ({_mcp_err}). "
        "MCP tools are unavailable over HTTP. "
        "You can still run the MCP server on stdio with: python -c "
        '"from metabook_py.mcp_server import mcp; mcp.run()"',
        stacklevel=1,
    )
