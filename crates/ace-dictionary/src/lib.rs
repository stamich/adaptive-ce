//! Dictionary abstractions (registry, identity and validation only).
//!
//! Training, adaptive reuse and semantic dictionaries are deferred; this crate defines the
//! stable seam ([`DictionaryProvider`]) that codecs will depend on.

mod dictionary;
mod provider;
mod registry;

pub use dictionary::*;
pub use provider::*;
pub use registry::*;
