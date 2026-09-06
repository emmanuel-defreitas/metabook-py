# Phase 0 Research: Schema-Agnostic Text-Fabric Reference Identifiers

All Text-Fabric signatures below were verified by introspecting an installed `text-fabric`
(`uv run --no-project --with text-fabric`), not recalled. No corpus was downloaded.

## R1 — Discovering the section hierarchy at runtime (FR-001)

**Decision**: Read `T.sectionTypes` from the loaded corpus; never name levels in code.

**Verified API**: `Text` exposes `sectionTypes`, `sectionTypeSet`, `sectionFeatures`,
`sectionFeatureTypes`, `languages`, `headings`. Lookup and inverse:

- `T.nodeFromSection(section, lang='en')`
- `T.sectionFromNode(n, lastSlot=False, lang='en', fillup=False, level=None)`
- `T.sectionTuple(n, lastSlot=False, fillup=False)`
- language-independent variants: `T.headingFromNode(n)`, `T.nodeFromHeading(head)`

**Rationale**: `sectionTypes` is the corpus's own answer to "what are my levels", so depth adapts
for free (1, 2, or 3+ levels — spec edge case) and Biblical vs secular corpora differ only in data.

**Alternatives rejected**: config file mapping corpus → level names (a second source of truth that
drifts); inferring depth from the reference string (unresolvable — `A:B` is ambiguous between a
2-level corpus and a partial path in a 3-level one).

## R2 — Unit lists and section anchoring (FR-011, FR-012, SC-005)

**Decision**: A unit of type `t` belongs to section `s` iff **its first slot lies in `s`'s slot range**.
Compute by walking `s`'s slots in order and, for each slot, taking `L.u(slot, otype=t)`, keeping a
unit only when that slot is its minimum slot. Result is canonical order by construction.

**Rationale**: Text-Fabric's `L.d(section, otype=t)` returns *embedded* nodes. A clause spanning
two verses is embedded in neither, so `L.d` silently drops it from both — which would violate
FR-012 and SC-005 (addressable from exactly one section). First-slot anchoring gives exactly one
home and makes resolution and serialization agree by using one definition in both directions.

**Verified API**: `E.oslots.s(node)` (slots of a node), `L.u(n, otype=None)`, `L.d(n, otype=None)`,
`F.otype.slotType`, `F.otype.maxSlot`, `F.otype.s(type)`, `N.sortNodes(nodeSet)`, `N.walk(nodes=None, events=False)`.

**Alternatives rejected**: `L.d` (drops spanning units — the exact case SC-005 tests);
"unit overlaps section" (a spanning unit would appear in two sections, breaking round-trip
identity and making indices ambiguous).

## R3 — Cache design (FR-017, SC-004)

**Decision**: `dict[(section_node, unit_type), tuple[int, ...]]` on the resolver instance, plus the
inverse `dict[(section_node, unit_type, node)] -> index` built lazily from the same tuple.
Cache is scoped to one adapter instance (one corpus identity), so it never needs invalidation.

**Rationale**: SC-004 asks for a constant factor over a single walk. Building a section's list is
O(slots in section); every subsequent index lookup and index computation in that section is O(1).
Serializing every node touches each section's builder once.

**Alternatives rejected**: global LRU keyed by corpus id (invalidation problem across versions for
no gain); precomputing all sections at load (pays for sections nobody references, and SC-004 only
requires amortized behaviour).

## R4 — Version resolution (FR-009, FR-010)

**Decision**: The adapter exposes `version`. For a TF app that is `A.version`. Resolution ignores
any version in the input; serialization and normalization always emit the adapter's version.

**Verified API**: the advanced app carries `version`, `versionGiven`, `versionOverride`;
`FabricCore` sets `self.version`.

**Rationale**: FR-010 is explicit — a mismatched input version is replaced *silently*. This makes
normalization idempotent (SC-002) and keeps stored references honest about which dataset produced them.

**Alternatives rejected**: warning on mismatch (spec forbids); resolving against the named version
(the library does not enumerate installed versions — spec assumption).

## R5 — Escaping (FR-004)

**Decision**: As fixed by the spec: quote a section value with `"` when it contains any of
`: / ! @`, whitespace, or `"`; double an embedded quote (`""`). Otherwise emit it bare.
One function pair, `escape_value` / `unescape_value`, used by both parser and formatter.

**Rationale**: Symmetry is the requirement (FR-004), so a single pair used in both directions is
the whole design. Common references (`Genesis:1:1`) stay bare and human-readable.

**Alternatives rejected**: percent-encoding (unreadable for CJK/Hebrew headings, and `%` then needs
escaping too); backslash escapes (two escaping systems once the URN form percent-encodes anyway).

## R6 — Testability and dependency direction (SC-001, SC-006)

**Decision**: Core depends on a `CorpusAdapter` Protocol, not on `tf`. Two implementations:
`tf_adapter.TextFabricAdapter` (optional import) and `tests/tf_ref/fake_corpus.py` (in-memory,
three levels, includes a unit that spans two innermost sections and headings in two languages).

**Rationale**: SC-006 forbids network in the test suite; a real TF corpus is a download. The fake
corpus also lets SC-005 (spanning unit) and the multilingual case be tested deterministically,
which a real corpus makes awkward. SC-001's "same code, different corpora" becomes a fixture
parameter instead of a claim.

**Alternatives rejected**: vendoring a tiny real corpus (licensing plus repo weight, and still
would not exercise a second level-naming scheme); mocking `tf` with `unittest.mock` (tests the mock).

## R7 — Property testing (spec assumption)

**Decision**: `hypothesis` if importable, else a parametrized corpus of generated references
committed as data. Invariants under test are exactly SC-002's four.

**Rationale**: The spec permits either. Keeping the invariants in one module means the fallback is
a different generator, not a different test.

## Resolved unknowns

| Unknown from Technical Context | Resolution |
|---|---|
| How is corpus depth known? | `T.sectionTypes` (R1) |
| Where does the version come from? | adapter `version` ← `A.version` (R4) |
| How are spanning units assigned? | first-slot anchoring (R2) |
| Is `text-fabric` a hard dependency? | No — optional extra behind the adapter (R6) |
| How do offline tests get a corpus? | in-memory fake adapter (R6) |

## Remaining ambiguities (FR-021 caps this list at five)

1. **Sibling headings that collide** — spec says "first in canonical order"; that makes some
   sections unaddressable. Documented, not fixed.
2. **Partial-depth selectors** — `Sec1!word2` counts units anchored anywhere under `Sec1`. Correct
   per spec assumption, but the index is only stable while the section's children are stable.
3. **`urn:tf:` reserved-character policy** — the grammar contract fixes percent-encoding for the URN
   form; whether that must match RFC 8141 r-components exactly is unresolved and unused here.
4. **Language fallback** — behaviour when a corpus lacks headings in the requested language
   (raise vs fall back to default) is not specified; contract picks raise, flagged for review.
5. **Multi-section spans** — explicitly out of scope; the grammar leaves `!` free for a future
   `Sec1:1:1!word3--Sec1:1:2!word2` without a breaking change.
