# Quickstart: Validating TF Reference Identifiers

How to prove the feature works end to end. Runnable scenarios only — implementation lives in
`src/tf_ref/`, task breakdown in `tasks.md`.

## Prerequisites

```bash
make setup                 # uv sync (repo already provides pytest)
uv sync --extra tf         # ONLY for the real-corpus smoke test; core needs none
```

The core library and its whole test suite must import and pass **without** `text-fabric` installed
and **without network access** (SC-006). If `pytest` needs the network, the design has regressed.

## Scenario 1 — One grammar, two corpora (SC-001, US1)

```bash
uv run pytest tests/tf_ref/test_resolve.py -v
```

Expected: the same parametrized tests pass against the synthetic fixture with levels
`("volume", "chapter", "paragraph")` and again with `("book", "scene", "line")`, with no
library code change — only the fixture differs. `Vol1:3:4` resolves to the paragraph the corpus
labels Vol1/3/4; `Vol1:3` resolves to the chapter (partial depth).

## Scenario 2 — Positional sub-units (US2)

```bash
uv run pytest tests/tf_ref/test_resolve.py -k selector -v
```

Expected: `Vol1:3:4!word2` is the second word anchored to that section;
`Vol1:3:4!word2-4` is three nodes in canonical order; index `0`, `N+1`, `word5-3`, and
`word3-clause1` each fail with the error kind the contract names — and the message quotes the
offending fragment (SC-003).

## Scenario 3 — Spanning units are addressable exactly once (SC-005, FR-012)

```bash
uv run pytest tests/tf_ref/test_units.py -k spanning -v
```

The fixture contains a clause whose text starts in section A and continues into B. Expected:
it is counted in A, absent from B's list, and serializes back to A. This is the case
Text-Fabric's `L.d` drops from both sections — see research.md R2.

## Scenario 4 — Round-trip invariants (SC-002, US3, US4)

```bash
uv run pytest tests/tf_ref/test_roundtrip.py -v
```

Expected: all four invariants hold over generated references — normalize idempotence,
`serialize∘resolve == normalize` for canonical single-node references, short↔URN identity, and
version-less input becoming version-explicit. A reference carrying a *stale* version normalizes
to the loaded dataset's version with no warning and no error (FR-010). A partial-depth selector
serializes to its canonical innermost section.

Runs under `hypothesis` when installed; falls back to parametrized cases otherwise. Both count.

## Scenario 5 — Escaping (US1 AS5, FR-004)

```bash
uv run pytest tests/tf_ref/test_grammar.py -k escap -v
```

Expected: headings containing `:`, `/`, `!`, `@`, whitespace, or `"` round-trip to the exact
original and re-serialize to the identical escaped form — `"Intro: Notes"`, `"say ""hi"""`.

## Scenario 6 — Cache keeps serialization cheap (SC-004, FR-017)

```bash
uv run pytest tests/tf_ref/test_units.py -k cache -v
```

Expected: serializing every word in the synthetic corpus scans that unit type **once** (assert on
a scan counter, not wall-clock), so a full walk stays within a constant factor of iterating the
nodes.

## Scenario 7 — Real corpus smoke test (SC-001, optional)

```bash
uv run pytest tests/tf_ref/test_tf_smoke.py -v      # skips when TF or the corpus is absent
```

Expected: on a locally present Biblical corpus, `Genesis:1:1` resolves and the resolved node
serializes back to `Genesis:1:1` with an explicit version. Absent corpus ⇒ **skipped, never failed**.

## Done when

- [ ] `uv run pytest tests/tf_ref -v` passes offline with `text-fabric` uninstalled
- [ ] Scenarios 1–6 green; Scenario 7 green or skipped
- [ ] `uv run ruff check src/tf_ref` and `uv run mypy src/tf_ref` clean (repo lint gates)
- [ ] Docs cover grammar, escaping, boundary policy, ≤5 ambiguities, and the multi-span extension note (FR-021)
