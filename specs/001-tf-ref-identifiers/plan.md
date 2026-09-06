# Implementation Plan: Schema-Agnostic Text-Fabric Reference Identifiers

**Branch**: `001-tf-ref-identifiers` | **Date**: 2026-09-06 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/001-tf-ref-identifiers/spec.md`

## Summary

One identifier grammar (`corpus@version/Sec1:Sec2:Sec3!word3-5`, plus a `urn:tf:` form) that
resolves to nodes in any Text-Fabric corpus by reading the corpus's own section hierarchy at
runtime. The library never hardcodes level names or depth: it asks the loaded corpus for its
section types and treats section values as opaque strings.

Technical approach: a small pure-Python core (grammar, escaping, `Reference` value object,
normalization) sitting on a narrow **corpus adapter protocol** — the six operations the core
needs (section types, version, languages, section lookup, section-of-node, slots-of-node).
Text-Fabric is one implementation of that protocol; an in-memory synthetic corpus is another.
That split is what makes SC-001 (same code, different corpora) and SC-006 (offline test suite)
structural rather than aspirational, and keeps `text-fabric` an optional dependency.

The one piece of real logic is the **unit list** (FR-011/FR-012): units of type `t` anchored to a
section by their *first slot*, in canonical order. It is computed from slot ranges rather than
Text-Fabric's embedding (`L.d`), because a unit spanning two verses is embedded in neither, and
cached per `(section node, unit type)` so repeated serialization does not rescan (FR-017, SC-004).

## Technical Context

**Language/Version**: Python 3.13+ (repo floor: `requires-python = ">=3.13"`)

**Primary Dependencies**: stdlib only for the core and grammar. `text-fabric>=13` is an **optional**
extra, imported only by the TF adapter. Dev: `pytest`, `pytest-asyncio` (already present);
`hypothesis` optional — property tests degrade to parametrized cases when it is absent (spec assumption).

**Storage**: N/A — reads a loaded corpus in memory; no persistence.

**Testing**: `pytest`. Round-trip invariants (SC-002) as property or parametrized tests over a
synthetic three-level corpus. Real-corpus smoke test is `skipif` when the corpus is not on disk.

**Target Platform**: Library, importable from CLI/API/notebook contexts on macOS/Linux. No network at runtime or test time.

**Project Type**: Single library (no service, no UI).

**Performance Goals**: SC-004 — after the first serialization warms a section's cache, serializing a
node is O(1) amortized; building one section's cache is O(slots in that section). Serializing every
node in a corpus stays within a constant factor of walking those nodes once.

**Constraints**: Offline (SC-006). No hardcoded level names (FR-001). Deterministic output: every
serialized reference carries an explicit version (FR-010).

**Scale/Scope**: Corpora up to ~10^6 slots (BHSA is ~426k words). Cache is bounded by
sections actually touched × unit types actually requested.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Checked against `.specify/memory/constitution.md` **v1.0.0**. Status: **passes**, re-verified after
Phase 1 design.

| Gate | Finding |
|---|---|
| I. Integration-First Contracts | No change to the published REST/MCP surface — this feature adds a new library that no endpoint imports. Its own public surface is pinned in `contracts/python-api.md` (types, four error kinds, guarantees) before implementation, so consumers get a stated contract from day one. Packaging changes (adding `src/tf_ref` to the wheel, `text-fabric` as an optional extra) are additive; no bump override needed. |
| II. Documentation Is Part of the Change | FR-021 already mandates grammar, escaping rule, boundary policy, ≤5 ambiguities, and the multi-span extension note. `contracts/grammar.md` carries the consumer-facing grammar and the rejection table; `quickstart.md` is runnable as written. README gains a section only if the library becomes part of the published product. |
| III. Downstream Propagation | No dependents affected. `MetaBookSDK`, the `MetaBook` app, and `example/` consume the HTTP API, not Python internals, and this feature changes no payload. If `tf_ref` later gains an HTTP surface, this gate re-opens and issues must be filed then. |
| IV. Test Discipline | Satisfied by construction, not by promise: the `CorpusAdapter` seam exists so the whole suite runs offline against an in-memory corpus (SC-006), the real-corpus smoke test skips rather than fails when the corpus is absent, and the four round-trip invariants (SC-002) plus the error-message rules (SC-003) are contract-shaped tests. |

## Scope corrections carried into this plan

Two spec assumptions do not hold in this repository. Both are recorded here rather than silently designed around.

1. **FR-019 / User Story 5 describe functions that do not exist.** A disk-wide search
   (`~/Desktop/Projects`, `~/.claude/skills`, `~/.agents`) finds no definition of `resolve_ref` or
   `node_to_ref`. There are no existing callers to keep working. They are therefore planned as
   **new thin wrappers** shaped the way the spec names them (so future callers match the spec), and
   US5 collapses from a compatibility constraint into two one-line delegations. **No back-compat risk exists.**

2. **This repository has no Text-Fabric code.** `metabook-py` is a FastAPI Book Structure API
   (Gutendex/EPUB/tokenizers/MongoDB); nothing imports `tf`. The nearest TF work on disk is
   `corpora-apps/corpora-cli/src/corpora_cli/reconcile/tfcorpus.py`, a stdlib-only `.tf` *file
   parser* for reconciliation — related domain, different job, different repo.

   The branch and spec live here, so the plan targets this repo, with the library kept free of any
   `metabook_py` import so it can be lifted into `corpora-apps` (or its own package) by moving one
   directory. **If the intended home is actually corpora-apps, say so before `/speckit-tasks` —
   only the Structure Decision below changes.**

## Project Structure

### Documentation (this feature)

```text
specs/001-tf-ref-identifiers/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── grammar.md       # Reference grammar, escaping rule, URN form
│   └── python-api.md    # Public functions, types, error taxonomy
├── checklists/
│   └── requirements.md  # Pre-existing
└── tasks.md             # Phase 2 output (/speckit-tasks — NOT created here)
```

### Source Code (repository root)

```text
src/
├── metabook_py/              # Existing app — untouched by this feature
└── tf_ref/                   # New, standalone, no metabook_py imports
    ├── __init__.py           # Public surface: parse, resolve, serialize, normalize, errors
    ├── grammar.py            # Short form + URN parsing, escaping/unescaping, formatting
    ├── reference.py          # Reference / Selector value objects, validation
    ├── adapter.py            # CorpusAdapter Protocol — the six operations the core needs
    ├── units.py              # Unit-list computation (first-slot anchoring) + per-section cache
    ├── resolver.py           # resolve / serialize / normalize against an adapter
    ├── errors.py             # ParseError, SectionNotFound, TypeNotInSection, IndexOutOfRange
    ├── tf_adapter.py         # CorpusAdapter over a Text-Fabric app (optional import of `tf`)
    └── legacy.py             # resolve_ref(ref_str, tf_app), node_to_ref(node, tf_app, corpus_id=None)

tests/
├── conftest.py               # Existing; gains the synthetic-corpus fixture
├── tf_ref/
│   ├── fake_corpus.py        # In-memory CorpusAdapter: 3 levels, spanning unit, multilingual headings
│   ├── test_grammar.py       # Parse/format, escaping round-trips, malformed input messages
│   ├── test_resolve.py       # Section + selector resolution, all four error kinds
│   ├── test_roundtrip.py     # SC-002 invariants (hypothesis if installed, else parametrized)
│   ├── test_units.py         # First-slot anchoring, spanning units, cache behaviour
│   └── test_tf_smoke.py      # skipif no real corpus / no text-fabric installed
```

**Structure Decision**: Single-project library at `src/tf_ref/`, a sibling of `src/metabook_py/`
rather than a subpackage, because it shares no domain with the Book Structure API and must stay
portable (see Scope corrections). Packaging note: `pyproject.toml`'s
`[tool.hatch.build.targets.wheel] packages = ["src/metabook_py"]` must gain `"src/tf_ref"`, and
`text-fabric` belongs in an optional extra (`[project.optional-dependencies] tf = ["text-fabric>=13"]`),
never in the base dependency list — the core and its tests must import without it.

## Complexity Tracking

> Constitution Check passes, so nothing here is a violation requiring justification. Recorded for
> review: the design adds exactly one indirection beyond the obvious, and its reason.

| Deliberate addition | Why needed | Simpler alternative rejected because |
|---|---|---|
| `CorpusAdapter` Protocol | SC-006 demands an offline test suite and SC-001 demands the same code on two unlike corpora | Calling `tf` directly makes every test require a downloaded corpus and makes "works on any corpus" untestable |
| Per-`(section, type)` unit cache | FR-017 / SC-004 | Recomputing per serialization is O(slots) per node — quadratic over a corpus walk |
