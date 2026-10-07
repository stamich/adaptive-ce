# Concept (AdaptiveCE_04_Koncepcja) -> implementation

| Concept § | Topic | Where | Status in 0.4.5 |
|---|---|---|---|
| 1 | Numeric type detection | `ace-analysis/numeric.rs`, `numeric_prefilter.rs` | done; U16 + lane-relative limits added |
| 2 | Delta-of-Delta | `ace-codecs/numeric.rs` (`NumericMode::DeltaOfDelta`) | done |
| 3 | ZigZag | `ace-bitpack` | done |
| 4 | Bit packing | `ace-bitpack` | done; u16 lane + faster bit I/O |
| 5 | Frame-of-Reference | `NumericMode::FrameOfReference` | done |
| 6 | Patched FOR "not right away" | — | **deferred to 0.4.1** (matches §47) |
| 7 | Numeric candidate family | `planner.rs::numeric_candidate` | done; injection guaranteed on NumericGeneral |
| 8 | Planner V4 | `ace-planner` (V4.3 routes) | done |
| 9 | Numeric fast-path | `route.rs`, `numeric_fast_decision_from_route` | done |
| 10 | Estimator V4 | `estimate_numeric` (exact) | done |
| 11–12 | Adaptive block policy / advisor | `ace-analysis/block_size.rs`, `BlockSizePolicy` | done; RandomAccess cap tested |
| 13 | Config | `AceConfig` | done |
| 14–15 | Format 1.3 / compatibility | `ace-format`, `docs/FORMAT-1.3.md` | done; additive width-2 note |
| 16 | BitPack metadata | NUM1 header | done |
| 18–19 | Endianness / signed | LE writer; ZigZag | done (BE telemetry only) |
| 20 | SIMD | scalar | not in scope |
| 21 / 46 | New crate `ace-bitpack` (`scalar`, `zigzag`, `for_codec`, `delta`, `delta_of_delta`) | `crates/ace-bitpack` | done; module names match §46 since 0.4.5-buildfix2 (+ generic `lane`) |
| 26–28 | Tests / property / fuzz | workspace tests (155), `fuzz/` | done; `numeric_roundtrip` added |
| 29–32 | Corpus V3 / numeric / ablation / block benchmarks | `examples/rust-benchmark` | present; `numeric-u16` corpus file -> 0.4.1 |
| 33 | Quality targets | measured below | see table |
| 39–40 | CLI / explain | `ace-cli` | `inspect` numeric telemetry added |
| 44 | Release gates | `README.md` | numeric ratio gates met (below) |

## Measured against §33 / §44 (corpus 8 MiB files, this container, min of 3)
| Target | Result |
|---|---|
| numeric-u32 ratio >= 3.0 | 2294x (fixed-step counter) |
| delta-series ratio >= 5.0 | 15.89x (FAST), 15.89x (BALANCED) |
| numeric-u32 compress >= 40 MB/s, decompress >= 100 MB/s | ~900 / ~870 MB/s |
| delta-series compress >= 50, decompress >= 150 MB/s | 77–87 / ~385 MB/s |
| mixed BALANCED ratio >= 3.45 | 3.96 (unchanged) |

Throughput figures come from a shared cloud container and are indicative, not the reference
benchmark; the formal release-gate run on the reference machine remains to be done.
