use ace_engine::{AceEngine, AceIndexedDecoder};
use std::io::Cursor;

/// Rejects arbitrary invalid magic without panicking.
#[test]
fn rejects_bad_magic() {
    let e = AceEngine::default_engine();
    assert!(e.decompress(b"NOPE-not-an-ace-file").is_err());
}

/// Detects corruption in the physical block region through parsing or reconstructed CRC32C.
#[test]
fn detects_block_corruption() {
    let e = AceEngine::default_engine();
    let mut encoded = e.compress(&vec![7u8; 100_000]).unwrap();
    let position = encoded.len().min(80).saturating_sub(1);
    encoded[position] ^= 0x55;
    assert!(e.decompress(&encoded).is_err());
}

/// Detects corruption of the serialized index or trailer when opening random access.
#[test]
fn detects_index_corruption() {
    let e = AceEngine::default_engine();
    let mut encoded = e.compress(&b"indexed data".repeat(100_000)).unwrap();
    let n = encoded.len();
    encoded[n - 10] ^= 0x44;
    assert!(
        AceIndexedDecoder::open(Cursor::new(encoded), ace_core::DecodeLimits::default()).is_err()
    );
}
