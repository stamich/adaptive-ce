use ace_format::{checksum, decode_file_header, encode_file_header, FileHeader, FORMAT_MINOR};

/// Verifies the ACE 0.4 writer emits Format 1.3 while preserving the fixed header shape.
#[test]
fn writer_emits_current_format_1_3() {
    assert_eq!(FORMAT_MINOR, 3);
    let header = FileHeader { minor_version: 3, flags: 0, default_block_size: 262_144, original_size: 123, block_count: 1 };
    let bytes = encode_file_header(&header);
    let decoded = decode_file_header(&bytes).unwrap();
    assert_eq!(decoded.minor_version, 3);
}

/// Verifies the ACE 0.4 reader still accepts a valid Format 1.2 fixed header.
#[test]
fn reader_accepts_1_2() {
    let header = FileHeader { minor_version: 3, flags: 0, default_block_size: 262_144, original_size: 0, block_count: 0 };
    let mut bytes = encode_file_header(&header);
    bytes[5] = 2;
    let crc = checksum(&bytes[..28]);
    bytes[28..32].copy_from_slice(&crc.to_le_bytes());
    assert_eq!(decode_file_header(&bytes).unwrap().minor_version, 2);
}

/// Verifies the ACE 0.4 reader still accepts a valid Format 1.1 fixed header.
#[test]
fn reader_accepts_1_1() {
    let header = FileHeader { minor_version: 3, flags: 0, default_block_size: 262_144, original_size: 0, block_count: 0 };
    let mut bytes = encode_file_header(&header);
    bytes[5] = 1;
    let crc = checksum(&bytes[..28]);
    bytes[28..32].copy_from_slice(&crc.to_le_bytes());
    assert_eq!(decode_file_header(&bytes).unwrap().minor_version, 1);
}
