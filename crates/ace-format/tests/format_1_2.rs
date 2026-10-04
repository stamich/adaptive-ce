use ace_format::{decode_file_header, encode_file_header, FileHeader, FORMAT_MINOR};

/// Verifies the 0.3 writer advertises minor format 1.2 while preserving the fixed header shape.
#[test]
fn writer_emits_format_1_2() {
    assert_eq!(FORMAT_MINOR, 2);
    let header = FileHeader { minor_version: 2, flags: 0, default_block_size: 262_144, original_size: 123, block_count: 1 };
    let bytes = encode_file_header(&header);
    let decoded = decode_file_header(&bytes).unwrap();
    assert_eq!(decoded.minor_version, 2);
}

/// Verifies the 0.3 reader still accepts a valid 1.1 fixed header after CRC recomputation.
#[test]
fn reader_accepts_1_1() {
    let header = FileHeader { minor_version: 2, flags: 0, default_block_size: 262_144, original_size: 0, block_count: 0 };
    let mut bytes = encode_file_header(&header);
    bytes[5] = 1;
    let crc = ace_format::checksum(&bytes[..28]);
    bytes[28..32].copy_from_slice(&crc.to_le_bytes());
    assert_eq!(decode_file_header(&bytes).unwrap().minor_version, 1);
}
