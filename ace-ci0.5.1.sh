#!/usr/bin/env bash
# Vendor-neutral CI entry point for ACE 0.5.1.
#
#   ./ace-ci0.5.1.sh pr        build gates + code audit + quick release-performance + demo
#   ./ace-ci0.5.1.sh release   full ace-release0.5.1.sh (needs ACE_AB_BASELINE_TREE)
#   ./ace-ci0.5.1.sh fuzz      all 15 fuzz targets, ACE_FUZZ_SECONDS each (default 60)
#   ./ace-ci0.5.1.sh msrv      check + test with Rust 1.97 (SKIPPED when not installed)
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/tools/ace-common0.5.1.sh"
cd "$ACE_ROOT"

case "${1:-}" in
  pr)
    ./ace-build0.5.1.sh --no-demo
    python3 tools/ace-code_audit0.5.1.py
    ./ace-benchmark0.5.1.sh release-performance --quick
    ./demo/ace-run-demo0.5.1.sh
    ;;
  release)
    [[ -n "${ACE_AB_BASELINE_TREE:-}" ]] || ace_die "set ACE_AB_BASELINE_TREE to the $ACE_BASELINE source tree"
    ./ace-release0.5.1.sh "$ACE_AB_BASELINE_TREE"
    ;;
  fuzz)
    if ! cargo +nightly fuzz --version >/dev/null 2>&1; then
      ace_log "fuzz: SKIPPED (cargo-fuzz / nightly toolchain not installed)"
      exit 0
    fi
    seconds="${ACE_FUZZ_SECONDS:-60}"
    for target in $(cargo +nightly fuzz list --fuzz-dir fuzz); do
      ace_step "fuzz $target (${seconds}s)"
      cargo +nightly fuzz run --fuzz-dir fuzz "$target" -- -max_total_time="$seconds"
    done
    ;;
  msrv)
    # The MSRV (1.97.0) is selected explicitly: rust-toolchain.toml pins the newer build
    # toolchain, so a plain `cargo` would not test the MSRV at all.
    toolchain=()
    if cargo +1.97.0 --version >/dev/null 2>&1; then
      toolchain=(+1.97.0)
    elif cargo +1.97 --version >/dev/null 2>&1; then
      toolchain=(+1.97)
    elif ! rustc --version | grep -q '^rustc 1\.97\.'; then
      ace_log "msrv: SKIPPED (Rust 1.97 not installed: rustup toolchain install 1.97.0)"
      exit 0
    fi
    cargo "${toolchain[@]}" check --workspace --all-targets --locked
    cargo "${toolchain[@]}" test --workspace --locked
    ;;
  *)
    ace_die "usage: $0 {pr|release|fuzz|msrv}"
    ;;
esac
ace_log "ci '$1' completed"
