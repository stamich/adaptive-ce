//! Deterministic synthetic workloads for ACE.
//!
//! One generator per workload, shared by unit/integration tests, the golden-file check, the
//! demo (`ace-corpus` binary) and the benchmark harness, so every consumer measures exactly the
//! same bytes. Generators never use randomness from the environment: the same name and size
//! always produce the same bytes on every platform.
//!
//! `generators` holds the 0.4.x corpus (Corpus V3); `float_generators` holds Corpus V4
//! (floating-point series and sparse-change integers, ACE 0.5.0).

mod float_generators;
mod generators;
mod rng;
mod workload;

pub use rng::*;
pub use workload::*;
