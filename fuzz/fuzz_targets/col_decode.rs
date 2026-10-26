#![no_main]

use std::io::Cursor;

use ace_core::{CodecId, DecodeLimits, EntropyCodecId};
use ace_engine::{AceEngine, AceIndexedDecoder};
use ace_format::{checksum, AceWriter, BlockHeader, FileHeader, FILE_FLAG_HAS_INDEX};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The fuzzer controls the `COL1` payload; the container around it is built so that the
    // column table parser and the inner-block checks are reached on every input. The first
    // four bytes choose the declared block size and the format minor version.
    let Some((control, payload)) = data.split_first_chunk::<4>() else {
        return;
    };
    let original_size = (u32::from_le_bytes([control[0], control[1], control[2], 0]) % (1 << 20)) + 1;
    let minor = 5 + (control[3] & 3); // 1.5 .. 1.8: below, at and above the first 1.6 reader
    let header = BlockHeader {
        block_id: 0,
        original_size,
        encoded_size: payload.len() as u32,
        metadata_size: 0,
        codec: CodecId::Columnar,
        entropy: EntropyCodecId::None,
        transforms: Vec::new(),
        dictionary: None,
        flags: 0,
        payload_crc32c: checksum(payload),
    };
    let mut writer = AceWriter::new(Vec::new());
    if writer
        .write_file_header(&FileHeader {
            minor_version: minor,
            flags: FILE_FLAG_HAS_INDEX,
            default_block_size: 1 << 20,
            original_size: u64::from(original_size),
            block_count: 1,
        })
        .is_err()
        || writer.write_block(&header, &[], payload).is_err()
    {
        return;
    }
    let Ok((file, _)) = writer.finish(true) else {
        return;
    };
    // Malformed input is expected to return an error. The invariants are that no reader panics,
    // none allocates without bound under the default limits, and a decode that succeeds returns
    // exactly the declared number of bytes.
    if let Ok(output) = AceEngine::default_engine().decompress(&file) {
        assert_eq!(output.len(), original_size as usize);
    }
    if let Ok(mut decoder) = AceIndexedDecoder::open(Cursor::new(&file), DecodeLimits::default()) {
        let _ = decoder.read_range(0..u64::from(original_size).min(4096));
        let _ = decoder.decode_block(0);
    }
});
