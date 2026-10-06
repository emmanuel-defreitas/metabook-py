"""Version of the installed package or a source-only deployment."""

import tomllib
from importlib.metadata import PackageNotFoundError, version
from pathlib import Path


def package_version() -> str:
    try:
        return version("metabook-py")
    except PackageNotFoundError:
        # Vercel can run the source directly after installing only dependencies.
        project = Path(__file__).resolve().parents[3] / "pyproject.toml"
        with project.open("rb") as file:
            return str(tomllib.load(file)["project"]["version"])
