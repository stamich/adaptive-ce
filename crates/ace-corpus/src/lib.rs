//! Deterministic synthetic workloads for ACE.
//!
//! One generator per workload, shared by unit/integration tests, the golden-file check, the
//! demo (`ace-corpus` binary) and the benchmark harness, so every consumer measures exactly the
//! same bytes. Generators never use randomness from the environment: the same name and size
//! always produce the same bytes on every platform.
//!
//! `generators` holds the 0.4.x corpus (Corpus V3); `float_generators` holds Corpus V4
//! (floating-point series and sparse-change integers, ACE 0.5.0); `v5_generators` holds Corpus V5
//! (integer series with small non-zero changes, ACE 0.6.1); `false_positive` holds the
//! Float lane false-positive corpus (not part of [`Workload`], so golden files ignore it);
//! `records` holds the record corpus of ACE 0.8.0 (tables of fixed-size records with their
//! layout, also outside [`Workload`]).
//!
//! **Stability:** internal building block of ACE (not published). Its public items exist for the
//! other ACE crates, the benchmark harness and the fuzz targets and may change in any release.
//! Use the `ace` crate, which is the public API (`docs/API-INVENTORY.md`).

mod false_positive;
mod float_generators;
mod generators;
mod records;
mod rng;
mod v5_generators;
mod workload;

pub use false_positive::*;
pub use records::*;
pub use rng::*;
pub use workload::*;
