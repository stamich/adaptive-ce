# ACE 0.4.6 — Architecture freeze (semantic freeze)

## Rule

0.4.6 freezes **behaviour**, not lines of code. A change is allowed when, and only when, it
leaves every observable output unchanged:

| Frozen | Guard |
|---|---|
| Format 1.3 wire format (writer), readers 1.0–1.3 | format tests, `examples/golden/0.4.6/GOLDEN.json` |
| Planner V4.3 decisions (route, plan, codec, entropy, parameters) | golden SHA-256 of every container |
| NUM1 payload layout | golden + `num1_properties` |
| Byte identity across threads, processes, API paths and SIMD backends | `determinism_matrix`, golden with `ACE_SIMD=scalar`, release script |
| Public API | additive changes only (`AceEngine::decompress_into`) |

Allowed: refactoring, documentation, tests, tooling, error handling that replaces panics with
errors on malformed input, decode-side speedups, benchmark methodology.

## Golden files

`examples/golden/0.4.6/GOLDEN.json` stores, for every `ace-corpus` workload (1 MiB each) and
every profile, the SHA-256 of the input, of the compressed container and of the decoded
output. `cargo test -p ace-engine --test golden` recomputes them.

* `input_sha256` changing → the generator changed (corpus bug, not a codec change).
* `ace_sha256` changing → a **semantic change**: not allowed in 0.4.x; belongs in 0.5.
* Regeneration (`ACE_GOLDEN_UPDATE=1`) is only legal together with a documented semantic
  change and a new golden directory.

The golden file was generated from the 0.4.5-buildfix2 code **before** any 0.4.6 change, and
the interleaved A/B additionally compares the 0.4.5-buildfix2 and 0.4.6 outputs byte for byte.

## Backend axis

`ACE_SIMD=scalar` (read once, `ace-simd`) forces the portable scan and CRC32C paths. Running
the golden test with and without it proves that AVX2 / SSE4.2 and scalar code produce the
same bytes on the same machine.

## What would break the freeze (examples)

* skipping generic analysis when the numeric margin dominates (may change planner choices);
* new NUM1 modes (Patched FOR), Gorilla/XOR, trained dictionaries;
* variable physical block size.

These are 0.5 candidates (`ROADMAP.md`).
