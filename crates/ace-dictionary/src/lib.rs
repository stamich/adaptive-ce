//! Dictionary abstractions (registry, identity and validation only).
//!
//! Training, adaptive reuse and semantic dictionaries are deferred; this crate defines the
//! stable seam ([`DictionaryProvider`]) that codecs will depend on.
//!
//! **Stability:** internal building block of ACE. Its public items exist for the other ACE
//! crates, the benchmark harness and the fuzz targets; they may change in any 0.x release.
//! Applications use `ace-engine`, `ace-stream` and `ace-core` (the 0.6 `ace` facade will
//! replace all three; see `docs/API-INVENTORY.md`).

mod dictionary;
mod provider;
mod registry;

pub use dictionary::*;
pub use provider::*;
pub use registry::*;
