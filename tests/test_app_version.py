"""Release metadata must be visible in the API's health and OpenAPI output."""

import tomllib
from importlib.metadata import PackageNotFoundError, version
from pathlib import Path

import httpx

from metabook_py.core.version import package_version
from metabook_py.main import app


async def test_health_and_openapi_use_package_version():
    async with httpx.AsyncClient(
        transport=httpx.ASGITransport(app=app), base_url="http://test"
    ) as client:
        health = await client.get("/health")
        schema = await client.get("/openapi.json")
    assert health.status_code == 200
    assert health.json()["version"] == version("metabook-py")
    assert schema.json()["info"]["version"] == version("metabook-py")


def test_source_deployment_reads_project_version_without_distribution(monkeypatch):
    def not_installed(name):
        raise PackageNotFoundError(name)

    monkeypatch.setattr("metabook_py.core.version.version", not_installed)
    project = Path(__file__).resolve().parents[1] / "pyproject.toml"
    with project.open("rb") as file:
        expected = tomllib.load(file)["project"]["version"]
    assert package_version() == expected
