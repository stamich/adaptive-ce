# Compile-safe benchmark JSON construction

## Problem

ACE 0.3-buildfix9 added enough planner telemetry that one large `serde_json::json!({...})`
expression exceeded Rust's default macro recursion limit. The runtime JSON was not recursive;
only compile-time macro expansion was.

## Resolution

The compilefix removes every object-shaped `json!({...})` invocation from the Rust benchmark
crate. Large documents are built incrementally with `JsonObjectBuilder`, which stores fields in
`serde_json::Map<String, Value>`.

Planner output is additionally split into small semantic sections:

```text
block detail:
  identity
  ranking
  quality
  hybrid
  outcome

planner summary:
  identity
  quality
  ranking
  planner work
  hybrid
  calibration
  timing
```

Sections are merged into the same flat JSON object. Therefore benchmark schema 1.9, field names,
and regression-tool expectations remain unchanged.

## Design rule

Future benchmark fields should be added to the appropriate builder section rather than expanding
a monolithic macro. This avoids depending on `#![recursion_limit = "..."]` and keeps serialization
code reviewable.

## Scope

This is a compile-only structural refactor. It does not alter:
- Planner V3.6 ranking;
- Hybrid LZ behavior;
- QualityEnvelope;
- codecs or entropy implementations;
- ACE Format 1.2;
- release-gate semantics.
