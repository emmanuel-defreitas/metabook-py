# Schema confidence and cleanup: local review notes

Repository: `emmanuel-defreitas/metabook-py` (public).
Feature branch: `feat/explainable-schema-cleanup`.
Base: `origin/dev`, `0443c313caf8f629f677e97c575e17719a13dd07`.
The repository's `.github/WORKFLOW.md` requires feature branches from `dev`.
No AGENTS.md or .agents/skills files were present in the checkout.

## Scope and public behavior

- Preserve schema names and rule selection priority. Add numeric heuristic
  support, per-schema candidate support, structural counts, automatic selection
  and override status to REST and MCP structure responses.
- Retain the confidence string; correct flat fallback to low and cap confidence
  for ambiguous signals or unsupported all-caps collections. Scores are not
  probabilities. All-caps matching now stays within a single line.
- Add optional enum `schema_override` to both REST analysis endpoints and both
  MCP analysis tools, reusing the existing detector and builder dispatch.
  Override changes the builder, while scores/evidence retain automatic meaning.
- Reuse and extend existing fetcher cleanup in a shared idempotent service.
  Detect and count the same cleaned body for fetched text and EPUB uploads.
  Recognise modern/wrapped starred and legacy Gutenberg end boundaries.
  Preserve HTML block boundaries needed for marker and contents recognition.
- Remove confirmed, explicitly labelled opening contents pages and preceding
  short metadata; retain uncertain front matter and real prose. Repeated body
  headings must match earlier contents entries before being treated as a body
  boundary. Dense and blank-separated contents layouts have regression tests.
- Evidence exposes counts only. No source excerpts or offset spans were added.
  Node indices remain cleaned-body ordinals, not download character offsets.

README.md documents consumption, exact scoring rules, errors and limitations;
Pydantic field descriptions and MCP tool descriptions document the surface.
The in-repo example displays arbitrary response JSON, so these additive fields
already appear without a decoder change; its API comments now document that.
Only comments were changed in the Rust example.

## Verification (2026-10-06, Python 3.13.5)

Commands used the repository's locked dependency environment. UV cache and
Python-install directories were set under /tmp because the default home cache
is not writable in this execution environment.

```bash
UV_CACHE_DIR=/tmp/metabook-uv UV_PYTHON_INSTALL_DIR=/tmp/metabook-python uv sync --locked
UV_CACHE_DIR=/tmp/metabook-uv UV_PYTHON_INSTALL_DIR=/tmp/metabook-python \
  PYTEST_ADDOPTS="-m 'not integration'" make ci
UV_CACHE_DIR=/tmp/metabook-uv UV_PYTHON_INSTALL_DIR=/tmp/metabook-python make pack
BASE=dev HEAD=feat/explainable-schema-cleanup \
  TITLE='feat: explain schema confidence and clean front matter' make pr-guard
```

- Ruff: all checks passed.
- mypy: no issues in 52 source files; existing notes about unchecked untyped
  tokenizer test bodies remain.
- pytest: **190 passed, 1 skipped, 1 deselected**. The skip is an absent optional
  local Text-Fabric corpus. The deselected test fetches a real Hugging Face
  tokenizer; the offline/mock tokenizer coverage passed.
- 29 new regression/contract cases cover cleanup, counts, preservation, license
  variants, compact HTML, schema-specific signals, ambiguity, weak and empty
  inputs, title/byline support, and REST/MCP override/evidence behavior.
- Source distribution and wheel built successfully under dist/ (ignored build
  artifacts, not committed). No version change or package publication.
- Branch guard, formatting of changed Python files, and git diff --check passed.

## Remaining limits and review/release work

This is ready for local code review. Marker-based inference remains heuristic;
verse-like numbers, typographic headings or unusual contents can mislead it.
Unlabelled front matter, prefaces, contents-only documents, sparse ambiguous
layouts and absent license boundaries are retained. There is no original-source
character-offset mapping. Corrected cleanup can change counts and body ordinals.
Existing storage scan fields are unchanged; detailed explanations are returned
in analysis responses rather than introducing a database migration.

Affected external dependents: `emmanuel-defreitas/MetaBookSDK` and the MetaBook
app. Review their handling of additive fields, changed flat confidence and body
counts. The repository constitution requires downstream issues before release;
file/link those during the authorised review/release step. No external issue,
PR, push, merge, release, deployment, or Reddit message was authorised or made.

Dialogue share with quoted spans represented by positions only, and sentence
length distributions, are recorded as future ideas and are outside this change.
