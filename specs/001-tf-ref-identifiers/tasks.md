# Tasks: Schema-Agnostic Text-Fabric Reference Identifiers

**Input**: Design documents in `specs/001-tf-ref-identifiers/`

**Tests**: Required by the project constitution. Each story starts with a failing offline test.

**Format**: `[ID] [P?] [Story] Description`

## Phase 1: Setup

- [x] T001 Add `src/tf_ref` to wheel packaging and a `tf` optional dependency in `pyproject.toml`
- [x] T002 Create the public package surface in `src/tf_ref/__init__.py`

## Phase 2: Foundation

- [x] T003 [P] Implement the four public error kinds in `src/tf_ref/errors.py`
- [x] T004 [P] Implement immutable `Reference` validation in `src/tf_ref/reference.py`
- [x] T005 [P] Define `CorpusAdapter` in `src/tf_ref/adapter.py`
- [x] T006 Build the deterministic three-level adapter in `tests/tf_ref/fake_corpus.py`

## Phase 3: User Story 1 — Parse and resolve corpus sections (P1)

**Goal**: One escaped reference grammar resolves full- and partial-depth sections without hardcoded level names.

**Independent test**: `Vol1:3:4` and `Vol1:3` resolve on two differently named synthetic hierarchies.

- [x] T007 [US1] Add escaping, malformed-input, and depth tests in `tests/tf_ref/test_grammar.py`
- [x] T008 [US1] Implement short-form parsing and formatting in `src/tf_ref/grammar.py`
- [x] T009 [US1] Add full-depth, partial-depth, and missing-section tests in `tests/tf_ref/test_resolve.py`
- [x] T010 [US1] Implement section resolution in `src/tf_ref/resolver.py`

## Phase 4: User Story 2 — Resolve positional units and ranges (P1)

**Goal**: Resolve 1-based units and inclusive ranges using first-slot section anchoring.

**Independent test**: `Sec1:1:1!word2-4` returns three canonical nodes; a spanning unit belongs only to its first section.

- [x] T011 [US2] Add selector, range, error, spanning-unit, and cache tests in `tests/tf_ref/test_resolve.py` and `tests/tf_ref/test_units.py`
- [x] T012 [US2] Implement cached first-slot unit lists in `src/tf_ref/units.py`
- [x] T013 [US2] Extend `resolve` for selectors and ranges in `src/tf_ref/resolver.py`

## Phase 5: User Story 3 — Serialize durable references (P2)

**Goal**: Serialize nodes with explicit loaded-corpus identity and normalize stale or missing versions.

**Independent test**: a stale-version input normalizes silently to the loaded version and a spanning node serializes to its first section.

- [x] T014 [US3] Add serialization, language, version, and normalization tests in `tests/tf_ref/test_roundtrip.py`
- [x] T015 [US3] Implement node serialization and normalization in `src/tf_ref/resolver.py`
- [x] T016 [US3] Implement canonical URN parsing and formatting in `src/tf_ref/grammar.py`

## Phase 6: User Story 4 — Prove round-trip guarantees (P2)

**Goal**: Verify normalization idempotence, serialize/resolve identity, and short/URN identity.

**Independent test**: all SC-002 invariants pass over deterministic generated references.

- [x] T017 [US4] Complete invariant coverage in `tests/tf_ref/test_roundtrip.py`
- [x] T018 [US4] Fix any invariant gaps in `src/tf_ref/grammar.py`, `src/tf_ref/units.py`, and `src/tf_ref/resolver.py`

## Phase 7: User Story 5 — Text-Fabric adapter and named wrappers (P3)

**Goal**: Expose the contracted `resolve_ref` and `node_to_ref` entry points without making Text-Fabric a base dependency.

**Independent test**: wrappers delegate through an adapter; the real-corpus smoke test skips when Text-Fabric or corpus data is absent.

- [x] T019 [US5] Implement the optional adapter in `src/tf_ref/tf_adapter.py`
- [x] T020 [US5] Implement thin wrappers in `src/tf_ref/legacy.py`
- [x] T021 [US5] Add wrapper tests and the optional smoke test in `tests/tf_ref/test_tf_smoke.py`

## Phase 8: Documentation and quality gates

- [x] T022 Document consumer usage and positional-stability limits in `README.md`
- [x] T023 Validate every command and example in `specs/001-tf-ref-identifiers/quickstart.md`
- [x] T024 Run `uv run pytest tests/tf_ref -v`, `uv run ruff check src/tf_ref tests/tf_ref`, and `uv run mypy src/tf_ref`
- [x] T025 Run the repository-wide `make ci`

## Dependencies

- T001–T006 block all user stories.
- US1 blocks US2; US1 and US2 block serialization and round-trip work.
- US3 blocks US4. US5 depends on the completed core but not on a real corpus.
- Documentation and repository-wide gates run after all selected stories.

## Delivery checkpoints

- **P1 MVP**: T001–T013 — schema-agnostic parse and bidirectional section/unit resolution.
- **Durable references**: T014–T018 — explicit versions and round-trip guarantees.
- **Text-Fabric integration**: T019–T025 — optional adapter, wrappers, docs, and full gates.
