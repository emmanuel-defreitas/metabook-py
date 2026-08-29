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

from metabook_py.core.config import settings
from metabook_py.routers.books import router as books_router

# ── Lifespan ───────────────────────────────────────────────────────────────────


@asynccontextmanager
async def lifespan(app: FastAPI):  # noqa: ANN201
    yield
    # Graceful shutdown: flush in-memory cache so tests don't bleed state
    from metabook_py.core.cache import book_text_cache

    book_text_cache.clear()


# ── App factory ────────────────────────────────────────────────────────────────

app = FastAPI(
    title=settings.app_name,
    version="1.0.0",
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


@app.get("/health", tags=["meta"])
async def health() -> dict:
    """Liveness probe."""
    from metabook_py.core.cache import book_text_cache

    return {
        "status": "ok",
        "version": "1.0.0",
        "cache_entries": book_text_cache.size,
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
