//! Shared model, errors, limits and physical-plan types of the Adaptive Compression Engine.
//!
//! **Stability:** part of the public ACE API (with `ace-engine`, `ace-stream` and `ace-core`).
//! 0.5.x releases change it only additively; the 0.6 `ace` facade will re-export it (see
//! `docs/API-INVENTORY.md`).

mod block;
mod codec;
mod config;
mod dictionary;
mod error;
mod limits;
mod numeric;
mod plan;
mod profile;
mod stats;
mod version;

pub use block::*;
pub use codec::*;
pub use config::*;
pub use dictionary::*;
pub use error::*;
pub use limits::*;
pub use numeric::*;
pub use plan::*;
pub use profile::*;
pub use stats::*;
pub use version::*;
