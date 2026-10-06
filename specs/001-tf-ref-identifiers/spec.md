# Feature Specification: Schema-Agnostic Text-Fabric Reference Identifiers

**Feature Branch**: `001-tf-ref-identifiers`
**Created**: 2026-09-06
**Status**: Draft
**Input**: User description: "Schema-agnostic reference-identifier library for Text-Fabric corpora that works unchanged on Biblical (book/chapter/verse) and secular (volume/chapter/paragraph, book/scene/line) corpora by reading the corpus's own section hierarchy at runtime."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Cite a passage in any corpus with one identifier syntax (Priority: P1)

A researcher or downstream tool wants to name a location in a corpus (e.g. "Genesis 1:1", "Volume 2, chapter 3, paragraph 4") using a single compact string, and have the library turn that string into the corpus's internal node without knowing in advance whether the corpus is Biblical or secular.

**Why this priority**: This is the core value: one identifier grammar for every corpus. Everything else (serialization, normalization, ranges) builds on parse + resolve.

**Independent Test**: Load a synthetic three-level corpus, parse `Vol1:3:4`, resolve it, and confirm the returned node is the paragraph node the corpus labels Vol1 / 3 / 4. Repeat on a corpus with different level names and confirm no code change is needed.

**Acceptance Scenarios**:

1. **Given** a corpus with three section levels, **When** a user submits `Sec1:Sec2:Sec3`, **Then** the innermost section node is returned.
2. **Given** a corpus with three section levels, **When** a user submits only `Sec1:Sec2`, **Then** the level-2 section node is returned (partial depth is allowed).
3. **Given** an identifier with more section components than the corpus has levels, **When** it is resolved, **Then** a parse error names the extra component.
4. **Given** a section heading that does not exist, **When** it is resolved, **Then** a "section not found" error names the missing heading and the level at which lookup failed.
5. **Given** a section value containing a space or a colon (e.g. a heading `Part One` or `Intro: Notes`), **When** the value is written using the documented escaping rule, **Then** it parses back to the exact original heading and re-serializes to the same escaped form.

---

### User Story 2 - Address a sub-unit inside a section by position (Priority: P1)

A user wants to point at "the second clause of this verse" or "words 3 through 5 of this paragraph" without knowing node numbers, using a selector such as `!clause2` or `!word3-5`.

**Why this priority**: Positional sub-unit selection is the main reason to have references rather than raw node IDs. It is required for the round-trip guarantees in P2.

**Independent Test**: On the synthetic corpus, resolve `Sec1:1:1!word2` and confirm it is the second word (in canonical order) whose first slot lies in that section. Resolve `Sec1:1:1!word2-4` and confirm a three-item list in canonical order.

**Acceptance Scenarios**:

1. **Given** a section with N units of a type, **When** the user selects index 1..N, **Then** the corresponding unit is returned.
2. **Given** a section with N units, **When** the user selects index N+1 or 0, **Then** an "index out of range" error states the requested index and the valid range 1..N.
3. **Given** a range selector `!wordA-B`, **When** resolved, **Then** a list of B−A+1 nodes is returned in canonical order.
4. **Given** a range where the second bound is smaller than the first, **When** parsed, **Then** a parse error is raised.
5. **Given** a selector naming a type that never occurs in the section, **When** resolved, **Then** a "type not in section" error names the type and section.
6. **Given** a range selector whose second part names a different type (e.g. `!word3-clause1`), **When** parsed, **Then** it is rejected as a cross-type range.
7. **Given** a unit whose text spans two adjacent innermost sections, **When** counting units in the first section, **Then** that unit is counted in the first section (where it begins) and not in the second.

---

### User Story 3 - Produce durable, version-explicit references from a node (Priority: P2)

A tool holding a node wants to store or display a reference string for it that will still mean the same thing later, even if the corpus is updated to a new version.

**Why this priority**: Stored references outlive the loaded dataset. Making the version explicit on output prevents silent drift.

**Independent Test**: Serialize a word node; confirm the output contains an explicit `@version`. Parse a reference without a version, normalize it, and confirm the output now contains the version of the loaded corpus.

**Acceptance Scenarios**:

1. **Given** any node, **When** it is serialized in short form, **Then** the output includes the corpus id and the version of the loaded dataset.
2. **Given** any node, **When** it is serialized in URN form, **Then** the output begins with `urn:tf:` and encodes the same information as the short form.
3. **Given** a reference string with no version, **When** normalized, **Then** the result is version-explicit.
4. **Given** a reference string with a version that does not match the loaded dataset, **When** normalized, **Then** the result carries the loaded dataset's version, replacing the input version silently (no warning, no error).
5. **Given** a node whose text begins in one innermost section and continues into the next, **When** serialized, **Then** the reference names the section where the node begins.
6. **Given** a corpus with multiple languages for section headings, **When** serialized with a specific language, **Then** the headings appear in that language; with no language given, the corpus's configured default is used.

---

### User Story 4 - Round-trip guarantees for stored references (Priority: P2)

A user who has stored references expects: normalizing twice equals normalizing once; serializing a
resolved canonical reference gives back the normalized form; converting to URN and back gives the
original. A selector on a partial-depth path resolves, but serialization canonicalizes it to the
innermost section because a bare resolved node cannot retain the broader input path.

**Why this priority**: These invariants are what make references safe to store, compare, and deduplicate.

**Independent Test**: Property-based or parametrized tests over generated references on the synthetic corpus asserting each invariant.

**Acceptance Scenarios**:

1. **Given** any valid reference r, **When** normalize(normalize(r)) is compared to normalize(r), **Then** they are identical.
2. **Given** any valid canonical single-node reference r, **When** serialize(resolve(r)) is compared to normalize(r), **Then** they are identical.
3. **Given** any valid reference, **When** converted to URN and back to short form, **Then** the original short form is recovered.
4. **Given** a range reference, **When** resolved and each returned node is serialized, **Then** the first and last serializations correspond to the range bounds.

---

### User Story 5 - Existing callers keep working (Priority: P3)

Code that already calls `resolve_ref(ref_str, tf_app)` and `node_to_ref(node, tf_app, corpus_id=None)` continues to work with no changes.

**Why this priority**: Backward compatibility is required but adds no new capability.

**Independent Test**: Call both legacy functions on the synthetic corpus and confirm they delegate to the new resolve/serialize behavior.

**Acceptance Scenarios**:

1. **Given** the legacy resolve function, **When** called with a valid reference and app, **Then** it returns the same node as the new resolver.
2. **Given** the legacy serialize function, **When** called with a node, **Then** it returns the same string as the new short-form serializer.

---

### Edge Cases

- Section headings containing `:`, `/`, `!`, `@`, whitespace, or the quote character itself must round-trip through the escaping rule.
- A reference with only a corpus id and no sections (`corpus@v/`) is invalid.
- A selector with no section path (`!word1` alone) is invalid; a selector always follows at least one section.
- Zero, negative, or non-numeric indices are parse errors, not range errors.
- A section with zero units of the requested type raises "type not in section", not "index out of range".
- Corpus id or version containing `/` or `:` is out of scope; the library documents that these must not contain grammar delimiters.
- Corpora with one or two section levels must work; the grammar depth adapts to the corpus.
- Positional indices are only stable within the same (corpus, version, language) triple; the library documents this and never claims cross-version stability.
- Serialization must be fast enough to call on every node in a large corpus without visibly degrading (see SC-004).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The library MUST determine the section hierarchy from the loaded corpus at runtime and MUST NOT hardcode level names or depth.
- **FR-002**: The library MUST accept references in the short form `[corpusId[@version]/]<Sec1>[:<Sec2>[:...]][!<otype><index>[-<index>]]`.
- **FR-003**: The library MUST accept and produce the canonical URN form `urn:tf:<corpusId>[@version]:<sections...>[!<selector>]`, and short form and URN form MUST convert to each other losslessly.
- **FR-004**: The library MUST define exactly one escaping rule for section values containing `:`, `/`, `!`, `@`, whitespace, or the escape delimiter, MUST document it, and MUST apply it symmetrically when parsing and when producing references.
- **FR-005**: The library MUST expose a structured reference value holding corpus id, optional version, section path, optional target type, start index, and optional end index.
- **FR-006**: Parsing MUST reject malformed input with an error that quotes the offending substring.
- **FR-007**: Resolving MUST return one node for a single reference and an ordered list of nodes for a range reference.
- **FR-008**: Selectors MUST use 1-based indices; ranges MUST be inclusive and MUST be of a single unit type. Cross-type ranges MUST be rejected at parse time.
- **FR-009**: When the version is omitted, resolving MUST use the latest version available to the loaded corpus; if the corpus exposes no version metadata, the loaded dataset MUST be treated as latest.
- **FR-010**: Serialization and normalization MUST always emit an explicit version, namely the version of the dataset actually used. An input version that differs from the loaded dataset MUST be replaced silently; the mismatch is neither reported nor treated as an error.
- **FR-011**: The unit list for a (section, unit type) pair MUST be defined as all units of that type whose first text slot lies within the section's slot range, in canonical order. Both resolution and serialization MUST use this same definition.
- **FR-012**: A unit whose text spans more than one innermost section MUST be anchored to the section containing its first slot, in both directions.
- **FR-013**: Serialization MUST determine a node's section from the innermost section of its first slot.
- **FR-014**: Serialization MUST support both short and URN output forms.
- **FR-015**: Normalization MUST parse, fill in the version, and re-serialize, so that normalization is idempotent.
- **FR-016**: Language selection for section headings MUST be supported on both resolve and serialize, defaulting to the corpus's configured language.
- **FR-017**: The library MUST cache per-(section, unit type) unit lists so that repeated serialization does not rescan the corpus.
- **FR-018**: The library MUST raise four distinct error kinds: parse error, section not found, type not in section, index out of range. Each message MUST include the offending fragment and, where applicable, the valid count or range.
- **FR-019**: The library MUST provide the two legacy entry points `resolve_ref(ref_str, tf_app)` and `node_to_ref(node, tf_app, corpus_id=None)` as thin wrappers over the new behavior.
- **FR-020**: The library MUST document that positional indices are stable only within a fixed (corpus id, version, language) triple.
- **FR-021**: Documentation MUST include the grammar, the escaping rule, the boundary policy, a list of remaining ambiguities (at most five), and a paragraph on extending to multi-section spans without changing the grammar.

### Key Entities

- **Reference**: A parsed identifier. Attributes: corpus id, version (may be absent on input, always present on output), ordered section path (1 to depth values), optional target unit type, start index, optional end index.
- **Section path**: The ordered heading values identifying a section at some depth of the corpus hierarchy. Values are opaque strings supplied by the corpus, possibly in several languages.
- **Selector**: A unit type plus a 1-based position or inclusive position range within the addressed section.
- **Unit list**: For a given section and unit type, the canonical-order list of units anchored to that section by their first slot. The basis for both index lookup and index computation.
- **Corpus identity**: (corpus id, version, language). The scope within which positions are stable.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The same library, with no code changes, passes its full test suite on a three-level synthetic corpus and, when available, a smoke test on a real Biblical corpus with different level names.
- **SC-002**: 100% of generated valid references satisfy normalization idempotence, URN↔short,
  and version-less → version-explicit; canonical single-node references additionally satisfy
  serialize∘resolve.
- **SC-003**: 100% of malformed inputs in the test corpus produce an error whose message contains the offending fragment; range/index errors additionally state the valid range.
- **SC-004**: Serializing every node in a corpus is at most a constant factor slower than iterating those nodes once, after the first serialization per section warms the cache.
- **SC-005**: A unit that spans two innermost sections is addressable from exactly one section (the one where it begins) and serializes back to that same section.
- **SC-006**: The library runs offline: the full test suite completes with no network access and no external corpus download.

## Assumptions

- Corpus id and version strings never contain `/`, `:`, `@`, or `!`; this is documented rather than escaped.
- Escaping rule: section values are wrapped in double quotes when they contain any delimiter, whitespace, or a double quote; an embedded double quote is doubled (`""`). Unquoted values are used otherwise. This choice was made because it keeps common references human-readable and is unambiguous in both directions.
- Section headings in the corpus are unique among siblings within a given language; if the corpus violates this, the first match in canonical order is used.
- "Latest version" for a loaded corpus is whatever version the app reports for itself; the library does not enumerate other installed versions.
- Canonical order is the corpus's native node order.
- Property tests use a property-testing library if installed, otherwise fall back to parametrized cases; both are acceptable.
- The Biblical smoke test is skipped, not failed, when the real corpus is not present locally.
- A partial-depth section path (fewer components than the corpus depth) is valid and resolves to the section at that depth; selectors on a non-innermost section count units anchored anywhere within that section's slot range.
- Multi-section spans (a range whose endpoints are in different sections) are out of scope for this feature and are only described as a future extension.
