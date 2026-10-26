//! Shared model, errors, limits and physical-plan types of the Adaptive Compression Engine.
//!
//! **Stability:** internal building block of ACE (not published). Its public items exist for the
//! other ACE crates, the benchmark harness and the fuzz targets and may change in any release.
//! Use the `ace` crate, which is the public API (`docs/API-INVENTORY.md`).

mod block;
mod codec;
mod config;
mod dictionary;
mod error;
mod limits;
mod numeric;
mod plan;
mod profile;
mod schema;
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
pub use schema::*;
pub use stats::*;
pub use version::*;
