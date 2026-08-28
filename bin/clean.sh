#!/usr/bin/env bash
#
# clean.sh
#
# Clean the project: remove caches, build artifacts, and venv.
#
# Usage:
#   ./bin/clean.sh
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

DIRS_TO_REMOVE=(".venv" ".mypy_cache" ".ruff_cache" ".pytest_cache" "dist" "build")

remove() {
  local path="$1"
  if [[ -e "$path" ]]; then
    rm -rf "$path"
    echo "  removed  $(basename "$path")"
  fi
}

echo "Cleaning project..."
echo

for name in "${DIRS_TO_REMOVE[@]}"; do
  remove "$ROOT/$name"
done

echo "Removing __pycache__ and *.egg-info directories..."
find "$ROOT" -type d -name __pycache__ -prune -exec rm -rf {} + 2>/dev/null || true
find "$ROOT" -type d -name '*.egg-info' -prune -exec rm -rf {} + 2>/dev/null || true

echo
echo "Done."
