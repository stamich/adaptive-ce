//! Deterministic synthetic workloads for ACE.
//!
//! One generator per workload, shared by unit/integration tests, the golden-file check, the
//! demo (`ace-corpus` binary) and the benchmark harness, so every consumer measures exactly the
//! same bytes. Generators never use randomness from the environment: the same name and size
//! always produce the same bytes on every platform.
//!
//! `generators` holds the 0.4.x corpus (Corpus V3); `float_generators` holds Corpus V4
//! (floating-point series and sparse-change integers, ACE 0.5.0); `false_positive` holds the
//! Float lane false-positive corpus (not part of [`Workload`], so golden files ignore it).
//!
//! **Stability:** internal building block of ACE. Its public items exist for the other ACE
//! crates, the benchmark harness and the fuzz targets; they may change in any 0.x release.
//! Applications use `ace-engine`, `ace-stream` and `ace-core` (the 0.6 `ace` facade will
//! replace all three; see `docs/API-INVENTORY.md`).

mod false_positive;
mod float_generators;
mod generators;
mod rng;
mod workload;

pub use false_positive::*;
pub use rng::*;
pub use workload::*;
