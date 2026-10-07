//! Canonical Huffman entropy coder.
//!
//! Metadata is a fixed 256-byte table of code lengths; the payload is an MSB-first bit stream.
//! Module map: `code_lengths` (deterministic tree construction), `canonical` (length table →
//! canonical codes and decoding tables), `bit_io` (MSB-first bit writer/reader), `encoder`,
//! `decoder`.

mod bit_io;
mod canonical;
mod code_lengths;
mod decoder;
mod encoder;

pub use decoder::*;
pub use encoder::*;
