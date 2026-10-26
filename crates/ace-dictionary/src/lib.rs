//! Dictionaries: the registry, the lookup seam and the deterministic trainer.
//!
//! [`DictionaryProvider`] is the seam that codecs and the engine depend on,
//! [`DictionaryRegistry`] the in-memory implementation, and [`train_dictionary`] builds a preset
//! dictionary for the LZ codec from sample inputs (ACE 0.8.0, `docs/FORMAT-1.6.md`).
//!
//! **Stability:** internal building block of ACE (not published). Its public items exist for the
//! other ACE crates, the benchmark harness and the fuzz targets and may change in any release.
//! Use the `ace` crate, which is the public API (`docs/API-INVENTORY.md`).

mod dictionary;
mod provider;
mod registry;
mod trainer;

pub use dictionary::*;
pub use provider::*;
pub use registry::*;
pub use trainer::*;
