#!/usr/bin/env bash
#
# publish.sh
#
# Publish a new release via GitHub Actions.
#
# Modes:
#   tag (default)
#     Bump version in pyproject.toml, commit, tag, push. The publish
#     workflow triggers on the tag.
#
#   --dispatch
#     Trigger workflow_dispatch on a branch (no version bump).
#
# Usage:
#   ./bin/publish.sh                    # patch
#   ./bin/publish.sh patch
#   ./bin/publish.sh minor
#   ./bin/publish.sh major
#   ./bin/publish.sh 1.2.3
#   ./bin/publish.sh --dispatch
#   ./bin/publish.sh --dispatch --publish
#   ./bin/publish.sh --dispatch --ref some-branch
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

PYPROJECT="$ROOT/pyproject.toml"

# Derived from the origin remote unless overridden.
REPO="${REPO:-$(cd "$ROOT" && git config --get remote.origin.url 2>/dev/null | sed -E 's,.*github\.com[:/],,; s,\.git$,,')}"
WORKFLOW_FILE="publish.yml"

# ── helpers ───────────────────────────────────────────────────────────────────

run() {
  echo "\$ $*"
  (cd "$ROOT" && "$@")
}

current_version() {
  local text
  text=$(cat "$PYPROJECT")
  if [[ "$text" =~ ^version[[:space:]]*=[[:space:]]*\"([^\"]+)\" ]]; then
    echo "${BASH_REMATCH[1]}"
  else
    echo "error: could not find version in pyproject.toml" >&2
    exit 1
  fi
}

bump() {
  local version="$1"
  local part="$2"
  IFS='.' read -r major minor patch <<< "$version"
  case "$part" in
    major) echo "$((major + 1)).0.0" ;;
    minor) echo "${major}.$((minor + 1)).0" ;;
    patch) echo "${major}.${minor}.$((patch + 1))" ;;
    *)     echo "$version" ;;
  esac
}

set_version() {
  local old="$1"
  local new="$2"
  local text
  text=$(cat "$PYPROJECT")
  local updated
  updated=$(python3 -c '
import sys
old, new, text = sys.argv[1], sys.argv[2], sys.stdin.read()
updated = text.replace(f"\"{old}\"", f"\"{new}\"", 1)
sys.stdout.write(updated)
' "$old" "$new" <<< "$text")

  if [[ "$updated" == "$text" ]]; then
    echo "error: version string not found in pyproject.toml" >&2
    exit 1
  fi
  printf '%s' "$updated" > "$PYPROJECT"
  echo "  updated pyproject.toml"
}

resolve_new_version() {
  local arg="$1"
  if [[ "$arg" =~ ^(patch|minor|major)$ ]]; then
    bump "$(current_version)" "$arg"
    return
  fi
  if [[ "$arg" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
    echo "$arg"
    return
  fi
  echo "error: '$arg' is not a valid version (expected X.Y.Z or patch/minor/major)" >&2
  exit 1
}

# ── tag mode ──────────────────────────────────────────────────────────────────

ensure_clean_tree() {
  local out
  out=$(cd "$ROOT" && git status --porcelain)
  local dirty=()
  while IFS= read -r line; do
    [[ "$line" =~ ^\?\? ]] && continue
    dirty+=("$line")
  done <<< "$out"

  if [[ ${#dirty[@]} -gt 0 ]]; then
    echo "error: working tree has uncommitted changes. Commit or stash them before publishing." >&2
    exit 1
  fi
}

commit_and_tag() {
  local version="$1"
  local tag="v${version}"
  run git add "$PYPROJECT"
  run git commit -m "chore: bump version to ${version}"
  run git tag "$tag"
  echo "$tag"
}

push_tag() {
  local tag="$1"
  local branch
  branch=$(cd "$ROOT" && git rev-parse --abbrev-ref HEAD)
  run git push origin "$branch"
  run git push origin "$tag"
  echo
  echo "Tag ${tag} pushed. GitHub Actions will build and publish the package."
  echo "Track progress at https://github.com/${REPO}/actions"
}

tag_release() {
  local version_arg="$1"
  ensure_clean_tree
  local old new tag
  old=$(current_version)
  new=$(resolve_new_version "$version_arg")
  echo "Bumping version: ${old} → ${new}"
  set_version "$old" "$new"
  tag=$(commit_and_tag "$new")
  push_tag "$tag"
}

# ── dispatch mode ─────────────────────────────────────────────────────────────

github_token() {
  local token="${GITHUB_TOKEN:-}"
  if [[ -z "$token" ]]; then
    cat >&2 <<'EOM'
error: GITHUB_TOKEN is required for --dispatch.
Set it in your environment.
Create a token at https://github.com/settings/tokens (repo + workflow scopes).
EOM
    exit 1
  fi
  echo "$token"
}

dispatch_workflow() {
  local publish="$1"
  local ref="${2:-main}"
  local token
  token=$(github_token)

  local url="https://api.github.com/repos/${REPO}/actions/workflows/${WORKFLOW_FILE}/dispatches"
  local payload
  payload=$(printf '{"ref":"%s","inputs":{"publish":"%s"}}' "$ref" "$(echo "$publish" | tr '[:upper:]' '[:lower:]')")

  echo "Dispatching '${WORKFLOW_FILE}' on '${ref}' (publish=${publish})..."

  local http_code
  http_code=$(curl -sS -o /tmp/publish_dispatch.out -w "%{http_code}" \
    -X POST "$url" \
    -H "Authorization: Bearer ${token}" \
    -H "Accept: application/vnd.github+json" \
    -H "Content-Type: application/json" \
    -H "X-GitHub-Api-Version: 2022-11-28" \
    -d "$payload" || true)

  if [[ "$http_code" != "204" && "$http_code" != "201" ]]; then
    echo "GitHub API error ${http_code}:" >&2
    cat /tmp/publish_dispatch.out >&2 || true
    exit 1
  fi

  echo "Workflow dispatched. Track progress at https://github.com/${REPO}/actions"
}

# ── main ──────────────────────────────────────────────────────────────────────

main() {
  local version="patch"
  local do_dispatch=false
  local do_publish=false
  local ref="main"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --dispatch) do_dispatch=true; shift ;;
      --publish)  do_publish=true; shift ;;
      --ref)      ref="$2"; shift 2 ;;
      -h|--help)
        cat <<'EOM'
Publish helper

  ./bin/publish.sh                 # bump patch, commit, tag, push
  ./bin/publish.sh patch
  ./bin/publish.sh minor
  ./bin/publish.sh major
  ./bin/publish.sh 1.2.3
  ./bin/publish.sh --dispatch
  ./bin/publish.sh --dispatch --publish
  ./bin/publish.sh --dispatch --ref some-branch
EOM
        exit 0
        ;;
      *)
        if [[ "$1" != -* ]]; then
          version="$1"
        fi
        shift
        ;;
    esac
  done

  if $do_publish && ! $do_dispatch; then
    echo "--publish requires --dispatch" >&2
    exit 1
  fi

  if $do_dispatch; then
    dispatch_workflow "$do_publish" "$ref"
  else
    tag_release "$version"
  fi
}

main "$@"
