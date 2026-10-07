//! Build script of `ace-bench`: records the toolchain and compilation fingerprint so every
//! benchmark document states exactly how the measured binary was built.
//!
//! Exposed to the crate as `ACE_BUILD_*` compile-time environment variables.

use std::process::Command;

/// Emits `cargo:rustc-env` fingerprint variables.
fn main() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let rustc_version = Command::new(&rustc)
        .arg("-V")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|text| text.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    emit("ACE_BUILD_RUSTC", &rustc_version);
    emit("ACE_BUILD_TARGET", &env_or_unknown("TARGET"));
    emit("ACE_BUILD_PROFILE", &env_or_unknown("PROFILE"));
    emit("ACE_BUILD_OPT_LEVEL", &env_or_unknown("OPT_LEVEL"));
    emit(
        "ACE_BUILD_TARGET_FEATURES",
        &env_or_unknown("CARGO_CFG_TARGET_FEATURE"),
    );
    emit(
        "ACE_BUILD_RUSTFLAGS",
        &env_or_unknown("CARGO_ENCODED_RUSTFLAGS").replace('\u{1f}', " "),
    );
    let profile = profile_settings(&env_or_unknown("PROFILE"));
    emit("ACE_BUILD_LTO", &profile.lto);
    emit("ACE_BUILD_CODEGEN_UNITS", &profile.codegen_units);
    println!("cargo:rerun-if-changed=../../Cargo.toml");
    println!("cargo:rerun-if-env-changed=RUSTFLAGS");
    println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
    println!("cargo:rerun-if-changed=build.rs");
}

/// Reads a build-script environment variable, defaulting to `"unknown"`.
fn env_or_unknown(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| "unknown".to_string())
}

/// Exposes `value` to the crate as `env!(name)`.
fn emit(name: &str, value: &str) {
    println!("cargo:rustc-env={name}={value}");
}

/// `lto` and `codegen-units` of one Cargo profile, as written in the workspace manifest.
struct ProfileSettings {
    /// `lto` value or `"default"`.
    lto: String,
    /// `codegen-units` value or `"default"`.
    codegen_units: String,
}

/// Reads `[profile.<name>]` from the workspace `Cargo.toml` (Cargo does not pass these
/// settings to build scripts). Unknown keys fall back to `"default"`.
fn profile_settings(name: &str) -> ProfileSettings {
    let manifest = std::fs::read_to_string("../../Cargo.toml").unwrap_or_default();
    // Build scripts see PROFILE=debug for the `dev` profile.
    let section = if name == "debug" { "dev" } else { name };
    let header = format!("[profile.{section}]");
    let mut settings = ProfileSettings {
        lto: "default".to_string(),
        codegen_units: "default".to_string(),
    };
    let mut inside = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line == header;
            continue;
        }
        if !inside {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let value = value.trim().trim_matches('"').to_string();
            match key.trim() {
                "lto" => settings.lto = value,
                "codegen-units" => settings.codegen_units = value,
                _ => {}
            }
        }
    }
    settings
}
