//! Build script of the `ace` CLI: records the source revision and the toolchain for
//! `ace version` (exposed as `ACE_CLI_*` compile-time environment variables).

use std::process::Command;

/// Emits the `cargo:rustc-env` variables.
fn main() {
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir("../..")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|text| text.trim().to_string())
    };
    let revision = git(&["rev-parse", "HEAD"]).filter(|text| !text.is_empty());
    let dirty =
        git(&["status", "--porcelain", "--untracked-files=no"]).map(|text| !text.is_empty());
    let revision = match (revision, dirty) {
        (Some(revision), Some(true)) => format!("{revision} (modified)"),
        (Some(revision), _) => revision,
        (None, _) => "unknown (not built from a git checkout)".to_string(),
    };
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let rustc = Command::new(rustc)
        .arg("-V")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|text| text.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=ACE_CLI_GIT_REVISION={revision}");
    println!("cargo:rustc-env=ACE_CLI_RUSTC={rustc}");
    println!(
        "cargo:rustc-env=ACE_CLI_TARGET={}",
        std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string())
    );
    println!(
        "cargo:rustc-env=ACE_CLI_PROFILE={}",
        std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string())
    );
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");
    println!("cargo:rerun-if-changed=build.rs");
}
