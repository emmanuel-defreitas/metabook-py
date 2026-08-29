"""
Error-mapping tests for Gutendex outages.

Verifies that httpx failures against the Gutendex API surface as a clean
{"error": "gutendex_unreachable"} payload — 504 for timeouts and 502 for
connection failures / upstream 5xx on the REST endpoint, and an error dict
from the MCP tool — instead of an unhandled 500.
"""

import httpx
import pytest
import respx

from metabook_py.core.config import settings
from metabook_py.main import app
from metabook_py.mcp_server import search_book_structure

GUTENDEX_URL = f"{settings.gutendex_base_url}/books/"


@pytest.fixture
def client() -> httpx.AsyncClient:
    return httpx.AsyncClient(transport=httpx.ASGITransport(app=app), base_url="http://testserver")


class TestStructureEndpoint:
    @respx.mock
    @pytest.mark.asyncio
    async def test_timeout_returns_504(self, client):
        respx.get(GUTENDEX_URL).mock(side_effect=httpx.ReadTimeout("read timed out"))

        resp = await client.get("/api/books/structure", params={"gutenberg_id": 1342})

        assert resp.status_code == 504
        assert resp.json()["detail"]["error"] == "gutendex_unreachable"

    @respx.mock
    @pytest.mark.asyncio
    async def test_connect_error_returns_502(self, client):
        respx.get(GUTENDEX_URL).mock(side_effect=httpx.ConnectError("connection refused"))

        resp = await client.get("/api/books/structure", params={"gutenberg_id": 1342})

        assert resp.status_code == 502
        assert resp.json()["detail"]["error"] == "gutendex_unreachable"

    @respx.mock
    @pytest.mark.asyncio
    async def test_upstream_500_returns_502(self, client):
        respx.get(GUTENDEX_URL).mock(return_value=httpx.Response(500, text="oops"))

        resp = await client.get("/api/books/structure", params={"gutenberg_id": 1342})

        assert resp.status_code == 502
        assert resp.json()["detail"]["error"] == "gutendex_unreachable"


class TestMcpTool:
    @respx.mock
    @pytest.mark.asyncio
    async def test_timeout_returns_error_dict(self):
        respx.get(GUTENDEX_URL).mock(side_effect=httpx.ReadTimeout("read timed out"))

        result = await search_book_structure(gutenberg_id=1342)

        assert result["error"] == "gutendex_unreachable"
        assert "timed out" in result["hint"]

    @respx.mock
    @pytest.mark.asyncio
    async def test_connect_error_returns_error_dict(self):
        respx.get(GUTENDEX_URL).mock(side_effect=httpx.ConnectError("connection refused"))

        result = await search_book_structure(gutenberg_id=1342)

        assert result["error"] == "gutendex_unreachable"
        assert "could not be reached" in result["hint"]
