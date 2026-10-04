use ace_engine::{AceEngine, AceIndexedDecoder};
use std::io::Cursor;

/// Demonstrates indexed single-block and logical-range reconstruction without decoding preceding blocks.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = (0u8..=255)
        .cycle()
        .take(8 * 1024 * 1024)
        .collect::<Vec<_>>();
    let engine = AceEngine::default_engine();
    let encoded = engine.compress(&data)?;
    let mut indexed =
        AceIndexedDecoder::open(Cursor::new(encoded), ace_core::DecodeLimits::default())?;
    let block = indexed.decode_block(7)?;
    let range = indexed.read_range(1_000_000..1_100_000)?;
    assert_eq!(range, data[1_000_000..1_100_000]);
    println!(
        "indexed blocks={} block7={} bytes requested-range={} bytes",
        indexed.block_count(),
        block.len(),
        range.len()
    );
    Ok(())
}
