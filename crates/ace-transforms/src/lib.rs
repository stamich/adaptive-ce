//! Reversible byte-level preprocessing transforms.
//!
//! [`apply_transform`] / [`invert_transform`] dispatch on [`ace_core::TransformId`].

mod delta;
mod dispatch;

pub use delta::*;
pub use dispatch::*;
