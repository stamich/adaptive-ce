//! `ace-bench [<family>|all|--list]`: the command-line entry point of the benchmark harness;
//! everything else lives in the `ace_bench` library.

use ace_bench::{run_family, select_families, FAMILIES};
use std::time::Instant;

/// Entry point: `ace-bench [<family>|all|--list]`. `ace-bench [<family>|all|--list]`.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let selection = std::env::args().nth(1).unwrap_or_else(|| "all".to_string());
    if selection == "--list" {
        for (name, _) in FAMILIES {
            println!("{name}");
        }
        return Ok(());
    }
    for (name, runner) in select_families(&selection)? {
        let started = Instant::now();
        let path = run_family(name, runner)?;
        println!(
            "[{name}] {:.1}s -> {}",
            started.elapsed().as_secs_f64(),
            path.display()
        );
    }
    Ok(())
}
