use ace_core::AceConfig;
use ace_engine::AceEngine;

/// Builds a deterministic heterogeneous corpus that exercises several ACE planner decisions.
fn mixed_demo_data() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend(std::iter::repeat(0u8).take(2 * 1024 * 1024));
    for i in 0..300_000u32 {
        data.extend_from_slice(&(i / 4).to_le_bytes());
    }
    for i in 0..40_000u32 {
        data.extend_from_slice(
            format!("{{\"id\":{i},\"status\":\"ACTIVE\",\"service\":\"graphnet\"}}\n").as_bytes(),
        );
    }
    let mut x = 0x1234_5678u32;
    for _ in 0..2 * 1024 * 1024 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        data.push((x & 0xff) as u8);
    }
    data
}

/// Runs the end-to-end ACE 0.3 demo and prints adaptive plan distribution plus round-trip status.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = mixed_demo_data();
    let mut config = AceConfig::default();
    config.threads = 4;
    let engine = AceEngine::new(config)?;
    let (encoded, stats) = engine.compress_with_stats(&data)?;
    let decoded = engine.decompress(&encoded)?;
    assert_eq!(decoded, data);
    println!("ACE 0.3 demo");
    println!(
        "input={} output={} ratio={:.3} blocks={}",
        stats.input_bytes,
        stats.output_bytes,
        stats.compression_ratio(),
        stats.block_count
    );
    println!(
        "plans raw={} rle={} lz={} delta={} huffman={} rans={}",
        stats.raw_blocks,
        stats.rle_blocks,
        stats.lz_blocks,
        stats.delta_blocks,
        stats.huffman_blocks,
        stats.rans_blocks
    );
    println!("round-trip: OK");
    Ok(())
}
