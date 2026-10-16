//! Dictionary abstractions (registry, identity and validation only).
//!
//! Training, adaptive reuse and semantic dictionaries are deferred; this crate defines the
//! stable seam ([`DictionaryProvider`]) that codecs will depend on.
//!
//! **Stability:** internal building block of ACE (not published). Its public items exist for the
//! other ACE crates, the benchmark harness and the fuzz targets and may change in any release.
//! Use the `ace` crate, which is the public API (`docs/API-INVENTORY.md`).

mod dictionary;
mod provider;
mod registry;

pub use dictionary::*;
pub use provider::*;
pub use registry::*;
