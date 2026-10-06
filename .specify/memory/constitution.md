<!--
SYNC IMPACT REPORT
Version change: (unratified template) → 1.0.0
Bump rationale: First ratified constitution. Every placeholder replaced with concrete,
enforceable rules; MAJOR baseline established.

Principles defined (4, matching the four themes supplied at ratification):
  - (new) I. Integration-First Contracts
  - (new) II. Documentation Is Part of the Change
  - (new) III. Downstream Propagation
  - (new) IV. Test Discipline
  Template slot PRINCIPLE_5 removed deliberately — no fifth theme was supplied, and an invented
  principle would be unenforceable.

Sections added:
  - Compatibility & Versioning (was [SECTION_2_NAME])
  - Development Workflow & Quality Gates (was [SECTION_3_NAME])
  - Governance (filled)

Templates requiring updates:
  ✅ .specify/templates/tasks-template.md — "Tests are OPTIONAL" contradicted Principle IV; updated
  ✅ .specify/templates/plan-template.md — Constitution Check now lists the four concrete gates
  ✅ .specify/templates/spec-template.md — reviewed, no change needed (principles constrain plans
     and PRs, not spec structure)
  ✅ .claude/skills/speckit-*/SKILL.md — reviewed, generic agent wording, no CLAUDE-only references
  ✅ README.md / .github/WORKFLOW.md — reviewed; WORKFLOW.md remains the operational source of
     truth for branching and release, referenced rather than duplicated here
  ✅ specs/001-tf-ref-identifiers/plan.md — its Constitution Check said "no ratified constitution,
     gates skipped"; re-evaluated against v1.0.0 and now passes all four gates

Follow-up TODOs: none.
-->

# metabook-py Constitution

## Core Principles

### I. Integration-First Contracts

The published HTTP and MCP surface is the product; internals are replaceable, the contract is not.

- Response and error shapes MUST be treated as a public contract. `MetaBookSDK`, the `MetaBook`
  app, and `example/` mirror these shapes in other languages and break silently when they drift.
- Changes to the surface MUST be additive by default. Removing or renaming a field, changing its
  type, changing an error code, or changing an endpoint path is a BREAKING change and MUST follow
  the Compatibility & Versioning rules below.
- Every endpoint, tool, field, and error code MUST be self-describing through the OpenAPI schema
  and the Pydantic models that generate it. An undocumented field is not part of the contract and
  MUST NOT be relied on by consumers.
- Errors MUST stay machine-readable: a stable `error` identifier plus a human message. Consumers
  branch on the identifier, so identifiers MUST NOT be reworded casually.
- New capability SHOULD be reachable from both the REST router and the MCP server unless the
  capability is meaningless in one of them; the shared service layer exists so the two cannot drift.

*Rationale*: this repository is never the only thing that ships. A field renamed here becomes a
decoding failure in a Swift client that no Python test can catch.

### II. Documentation Is Part of the Change

A consumer-facing change is incomplete until a consumer could adopt it without reading the source.

- Any PR that adds, changes, or removes public behaviour MUST update the consumer-facing docs in
  the same PR: `README.md` for capability-level changes, endpoint/tool descriptions and field
  docstrings for surface changes, `.env` documentation for new configuration.
- Documentation MUST describe consumption — request, response, failure modes, required
  configuration — before it describes implementation.
- Every new environment variable MUST document its default and what happens when it is unset. A
  feature that silently no-ops without configuration MUST say so where the feature is documented.
- Examples in docs MUST be runnable as written against a local instance.

*Rationale*: integration cost is paid by the reader, and the reader is usually a different
codebase in a different language.

### III. Downstream Propagation

Dependent repositories MUST NOT discover a breaking change by failing.

- When a change alters the published surface, the author MUST identify every affected dependent —
  at minimum `emmanuel-defreitas/MetaBookSDK`, the `MetaBook` app, and the in-repo `example/`
  client — and, for each one, either update it in the same change (when it lives in this repo) or
  file a GitHub issue on that repository before the release lands.
- Each filed issue MUST name the change, the version that carries it, and the concrete migration
  the dependent needs. A link to the PR alone is insufficient.
- The originating PR MUST link every issue it filed, so the release has a complete record of
  downstream work outstanding.
- In-repo consumers (`example/`) MUST be updated in the same PR, never deferred to an issue.

*Rationale*: the SDK cannot be updated by this repository's CI, so the handoff has to be explicit
and traceable or it does not happen.

### IV. Test Discipline

Behaviour that is not tested is not shipped.

- Every behaviour change MUST ship with tests in the same PR. Tests are NOT optional and are NOT
  contingent on a specification requesting them.
- Contract-shaped behaviour (response payloads, error identifiers, status codes) MUST be covered by
  tests that assert the shape a consumer sees, not only internal state.
- The test suite MUST pass offline and deterministically. Tests MUST NOT require network access,
  downloaded corpora, or live third-party services; external services MUST be faked or mocked at
  the boundary (the existing `respx` pattern for HTTP).
- A test that requires an optional local resource MUST skip, never fail, when the resource is
  absent.
- Bug fixes MUST add the regression test that fails before the fix, whenever the failure can be
  reproduced deterministically.

*Rationale*: consumers in other languages cannot see regressions here until they are already
broken; the suite is the only place a contract break can be caught cheaply.

## Compatibility & Versioning

- Released versions follow semantic versioning as recorded in `pyproject.toml`.
- `.github/WORKFLOW.md` remains the operational source of truth for branching, promotion, and
  release mechanics. This constitution constrains *classification*, not mechanics.
- The promote job classifies bumps from line-count churn. Churn does not know what is breaking, so:
  **any change that breaks the published contract MUST force a `major` bump via the
  `workflow_dispatch` override, regardless of how small the diff is.** A one-line field rename is a
  MAJOR release.
- A BREAKING change MUST carry a migration note in the release description stating what changed and
  what dependents must do.
- Deprecation is preferred to removal: mark the old surface deprecated in its documentation, keep it
  working for at least one MINOR release, then remove it in a MAJOR release.

## Development Workflow & Quality Gates

- `make ci` (lint + test) MUST pass before merge; it is the automated floor, not the standard.
- Lint and type gates (`ruff`, `mypy`) MUST pass with no new suppressions. A new `# type: ignore` or
  `# noqa` MUST carry a comment explaining why.
- Every PR MUST be able to answer, in its description:
  1. Does this change the published surface? (Principle I — and if yes, is the bump forced?)
  2. Which docs were updated? (Principle II)
  3. Which dependents were updated or issued? (Principle III)
  4. Which tests cover it? (Principle IV)
- A PR that answers "none" to 2, 3, or 4 while changing public behaviour MUST be rejected or must
  record the justification in the PR description.
- Feature work driven by `/speckit-*` commands MUST record its Constitution Check in `plan.md`
  against these four principles.

## Governance

This constitution supersedes ad-hoc practice. Where it conflicts with habit, the constitution wins;
where it conflicts with `.github/WORKFLOW.md` on release *mechanics*, WORKFLOW.md wins.

- **Amendments** MUST be made by editing this file in a PR that states the rationale, the version
  bump, and the propagation performed across `.specify/templates/` and runtime guidance docs.
- **Constitution versioning** is semantic and independent of the product version:
  - MAJOR — a principle is removed or redefined in a backward-incompatible way.
  - MINOR — a principle or governing section is added, or guidance is materially expanded.
  - PATCH — clarification, wording, or typo fixes that do not change what is required.
- **Compliance review**: reviewers MUST verify the four PR questions above. Complexity that
  violates a principle MUST be justified in writing in the plan's Complexity Tracking table, or the
  change MUST be simplified.
- **Runtime guidance**: `README.md` documents consumption, `.github/WORKFLOW.md` documents release
  mechanics, and `specs/<feature>/plan.md` documents per-feature compliance.

**Version**: 1.0.0 | **Ratified**: 2026-09-06 | **Last Amended**: 2026-09-06
