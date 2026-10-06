# Contract: Reference Grammar

Normative for FR-002, FR-003, FR-004, FR-008. Both forms carry the same information and convert
losslessly (FR-003).

## Short form

```ebnf
reference   = [ corpus [ "@" version ] "/" ] sections [ "!" selector ] ;
corpus      = 1*( %x21-7E - ( "/" / ":" / "@" / "!" ) ) ;   (* documented, not escaped *)
version     = corpus ;
sections    = value *( ":" value ) ;                        (* 1..corpus depth *)
selector    = otype index [ "-" index ] ;
otype       = 1*( ALPHA / DIGIT / "_" ) ;
index       = 1*DIGIT ;                                     (* 1-based, no sign, no zero *)
value       = bare / quoted ;
bare        = 1*( %x21-10FFFF - ( ":" / "/" / "!" / "@" / DQUOTE / WSP ) ) ;
quoted      = DQUOTE *( ( %x20-10FFFF - DQUOTE ) / DQUOTE DQUOTE ) DQUOTE ;
```

Examples: `Genesis:1:1` · `bhsa@2021/Genesis:1:1!word3` · `Vol1:3:4!clause2-5` ·
`corpus@v1/"Part One":"Intro: Notes"` · `Genesis:1` (partial depth, addresses the chapter).

## Escaping rule (FR-004)

A section value is emitted **quoted** iff it contains any of `:` `/` `!` `@`, any whitespace, or `"`.
Inside quotes, `"` is doubled. Otherwise it is emitted bare. Parsing applies the inverse.

The same function pair serves parse and format, so `unescape(escape(v)) == v` for every string,
including `Part One`, `Intro: Notes`, `say "hi"`, and values containing `/`, `!`, `@`.

## URN form

```ebnf
urn = "urn:tf:" corpus [ "@" version ] ":" urn-sections [ "!" selector ] ;
urn-sections = urn-value *( ":" urn-value ) ;
urn-value    = 1*( unreserved / pct-encoded ) ;   (* RFC 3986 unreserved; ":" "/" "!" "@" and
                                                     whitespace are percent-encoded *)
```

`urn:tf:bhsa@2021:Genesis:1:1!word3`. Short↔URN is a value-encoding change only: parse either form
to the same `Reference`, and the round-trip `short → urn → short` is the identity (SC-002-3).

## Rejected at parse time (FR-006 — message quotes the offending substring)

| Input | Why |
|---|---|
| `A:B:C:D` on a 3-level corpus | more sections than depth — error names `D` (US1 AS3) |
| `corpus@v1/` | no sections |
| `!word1` | selector with no section path |
| `A:B!word0`, `A:B!word-1`, `A:B!wordx` | zero / negative / non-numeric index |
| `A:B!word5-3` | descending range (US2 AS4) |
| `A:B!word3-clause1` | cross-type range (FR-008, US2 AS6) |
| `A:"unterminated` | unbalanced quote |

Depth-dependent rejection (row 1) happens when the corpus is known; pure syntax errors need no corpus.

## Stability

Positional indices are stable only within one `(corpus id, version, language)` triple (FR-020).
Multi-section spans are out of scope; `!` is left free for a future `…!word3--…!word2` (FR-021).
