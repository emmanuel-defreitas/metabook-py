# Phase 1 Data Model

Entities from the spec, with the fields and rules the implementation needs. All are immutable
value objects (`@dataclass(frozen=True, slots=True)`) except the cache.

## Reference

A parsed identifier. The single structured value FR-005 requires.

| Field | Type | Rules |
|---|---|---|
| `corpus_id` | `str \| None` | May be absent on input (bare `Sec1:Sec2`). Must not contain `/ : @ !` (documented, not escaped). Always present on output. |
| `version` | `str \| None` | Absent on input ⇒ loaded dataset's version (FR-009). Always present after normalize/serialize (FR-010). Same character restriction as `corpus_id`. |
| `sections` | `tuple[str, ...]` | 1 ≤ len ≤ corpus depth. Empty is invalid (`corpus@v/` — spec edge case). Values are opaque corpus strings; length > depth is a parse error naming the extra component (US1 AS3). |
| `otype` | `str \| None` | Unit type of the selector. `None` ⇔ reference addresses a section. |
| `start` | `int \| None` | 1-based. Present iff `otype` is. Zero/negative/non-numeric are **parse** errors, not range errors (spec edge case). |
| `end` | `int \| None` | Inclusive upper bound of a range. `None` ⇒ single unit. `end < start` is a parse error (US2 AS4). |

**Derived**: `is_range = end is not None`; `is_section = otype is None`.

**Cross-field rules**
- `start`/`end` without `otype`, or `otype` without `start`, are parse errors.
- A selector requires at least one section — `!word1` alone is invalid (spec edge case).
- A range must be single-type: `!word3-clause1` is rejected at parse time (FR-008, US2 AS6).

## Section path

The `sections` tuple above, positionally aligned with the corpus's `sectionTypes`. Depth is the
corpus's, discovered at runtime (FR-001) — the library stores values only, never level names.
Partial depth is legal and addresses the section at that depth (US1 AS2).

## Selector

`(otype, start, end)` from the Reference. 1-based, inclusive, single-type (FR-008). Resolution
against a section yields one node (`end is None`) or `end - start + 1` nodes in canonical order (FR-007).

## Unit list

The pivot of the whole feature (FR-011). For a `(section node, unit type)` pair: every unit of that
type **whose first slot lies in the section's slot range**, in canonical order.

- Used in *both* directions: index → node (resolve) and node → index (serialize). One definition,
  so round-trips close (SC-002, SC-005).
- A unit spanning two sections belongs to the one holding its first slot, in both directions (FR-012).
- Empty list ⇒ `TypeNotInSection`, never `IndexOutOfRange` (spec edge case).
- Cached per pair on the resolver (FR-017), scoped to one adapter instance, so no invalidation.

## Corpus identity

`(corpus_id, version, language)` — the scope in which indices are stable (FR-020, SC-002).
The library documents that positions do not survive a change to any of the three; it never claims
cross-version stability.

## Corpus adapter (the port)

Not a spec entity; the seam that makes FR-001 and SC-006 work. Six operations:

| Operation | Returns | Backed by (Text-Fabric) |
|---|---|---|
| `section_types()` | `tuple[str, ...]` | `T.sectionTypes` |
| `version()` | `str` | `A.version` |
| `languages()` / `default_language()` | codes | `T.languages` |
| `node_from_section(values, lang)` | node \| `None` | `T.nodeFromSection(section, lang=…)` |
| `section_from_node(node, lang)` | `tuple[str, ...]` | `T.sectionFromNode(n, lang=…)` |
| `slots(node)` / `units_of_type(otype)` | slot ids / nodes | `E.oslots.s`, `F.otype.s` |

## State transitions

References are immutable; "transitions" are pure functions.

```text
str --parse--> Reference(version maybe None)
Reference --normalize--> Reference(version always set)   # idempotent (FR-015, SC-002-1)
Reference --resolve--> node | [node, ...]                 # FR-007
node --serialize--> Reference --format--> str             # short | urn (FR-014)
```

Invariants (SC-002): `normalize∘normalize = normalize`; `serialize∘resolve = normalize` for
single-node references; `short → urn → short` is identity.
