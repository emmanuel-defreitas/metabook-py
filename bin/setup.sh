#!/usr/bin/env bash
#
# setup.sh
#
# Installation script for the project.
#
# Usage:
#   ./bin/setup.sh
#

set -euo pipefail

ensure_tool() {
  local name="$1"
  local hint="${2:-}"
  local install_cmd="${3:-}"

  if ! command -v "$name" >/dev/null 2>&1; then
    if [[ -n "$install_cmd" ]]; then
      echo "\$${name} is not on PATH. Installing..."
      eval "$install_cmd"
    fi
    if ! command -v "$name" >/dev/null 2>&1; then
      echo "error: \`${name}\` is required on PATH. ${hint}" >&2
      exit 1
    fi
  fi
}

main() {
  ensure_tool \
    "uv" \
    "Install from https://docs.astral.sh/uv/" \
    'curl -LsSf https://astral.sh/uv/install.sh | sh'

  echo
  echo "Syncing dependencies..."
  uv sync

  echo
  echo "Setup complete."
}

main "$@"
