# Contract: Python API

Public surface of `tf_ref`. Signatures are the contract; bodies are implementation.

## Types

```python
@dataclass(frozen=True, slots=True)
class Reference:
    corpus_id: str | None
    version: str | None
    sections: tuple[str, ...]
    otype: str | None = None
    start: int | None = None
    end: int | None = None

    @property
    def is_range(self) -> bool: ...
    @property
    def is_section(self) -> bool: ...
```

```python
class CorpusAdapter(Protocol):
    """The narrow corpus surface needed by the offline core (FR-001, SC-006)."""
    def corpus_id(self) -> str: ...
    def section_types(self) -> tuple[str, ...]: ...
    def version(self) -> str: ...
    def default_language(self) -> str: ...
    def node_type(self, node: int) -> str: ...
    def node_from_section(self, values: Sequence[str], lang: str) -> int | None: ...
    def section_from_node(self, node: int, lang: str) -> tuple[str, ...]: ...
    def slots(self, node: int) -> tuple[int, ...]: ...
    def units_of_type(self, otype: str) -> Iterable[int]: ...
```

## Functions

```python
def parse(text: str, *, depth: int | None = None) -> Reference: ...
def format_ref(ref: Reference, *, urn: bool = False) -> str: ...

def resolve(ref: Reference | str, corpus: CorpusAdapter, *,
            lang: str | None = None) -> int | list[int]: ...          # FR-007

def serialize(node: int, corpus: CorpusAdapter, *, otype: str | None = None,
              corpus_id: str | None = None, lang: str | None = None,
              urn: bool = False) -> str: ...                          # FR-013, FR-014

def normalize(ref: Reference | str, corpus: CorpusAdapter, *,
              lang: str | None = None, urn: bool = False) -> str: ... # FR-015
```

- `resolve` returns one node for a single reference, an ordered list for a range (FR-007).
- `serialize` finds the section from the innermost section of the node's **first slot** (FR-013),
  and always emits an explicit version (FR-010).
- `normalize` = parse → fill version → re-serialize; idempotent (SC-002-1).
- `lang=None` uses the corpus's configured default (FR-016).

## Legacy wrappers (FR-019)

```python
def resolve_ref(ref_str: str, tf_app) -> int | list[int]: ...
def node_to_ref(node: int, tf_app, corpus_id: str | None = None) -> str: ...
```

Thin delegations over `resolve` / `serialize` with `TextFabricAdapter(tf_app)`. **No existing
callers** — see plan.md Scope corrections — so these are a naming contract for future callers,
not a compatibility layer.

## Errors (FR-018 — exactly four kinds)

```python
class RefError(Exception): ...
class ParseError(RefError): ...           # quotes the offending substring
class SectionNotFound(RefError): ...      # names missing heading + level of failure
class TypeNotInSection(RefError): ...     # names type + section
class IndexOutOfRange(RefError): ...      # states requested index + valid range 1..N
```

Message rules (SC-003): every message contains the offending fragment; `IndexOutOfRange`
additionally states the valid range. A section with zero units of the type raises
`TypeNotInSection`, never `IndexOutOfRange`.

## Guarantees

| ID | Guarantee |
|---|---|
| SC-002-1 | `normalize(normalize(r)) == normalize(r)` |
| SC-002-2 | `serialize(resolve(r)) == normalize(r)` for canonical single-node `r`; partial-depth selectors serialize to their innermost section |
| SC-002-3 | `parse(format_ref(parse(s), urn=True))` re-formats to `s` |
| SC-002-4 | version-less input normalizes to version-explicit output |
| SC-005 | a unit spanning two sections is addressable from exactly one and serializes back to it |
