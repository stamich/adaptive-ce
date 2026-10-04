use ace_core::AceConfig;
use ace_engine::AceEngine;

/// Demonstrates that different worker counts produce an identical deterministic ACE bitstream.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = b"parallel ACE deterministic physical plan\n".repeat(250_000);
    let mut one = AceConfig::default();
    one.threads = 1;
    let mut many = one.clone();
    many.threads = 8;
    let single = AceEngine::new(one)?.compress(&data)?;
    let parallel = AceEngine::new(many)?.compress(&data)?;
    assert_eq!(single, parallel);
    println!(
        "1-thread and 8-thread outputs are bit-for-bit identical: {} bytes",
        single.len()
    );
    Ok(())
}
