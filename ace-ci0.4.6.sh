#!/usr/bin/env bash
# Vendor-neutral CI entry point for ACE 0.4.6.
#
#   ./ace-ci0.4.6.sh pr        build gates + code audit + quick release-performance + demo
#   ./ace-ci0.4.6.sh release   full ace-release0.4.6.sh (needs ACE_AB_BASELINE_TREE)
#   ./ace-ci0.4.6.sh fuzz      all 12 fuzz targets, ACE_FUZZ_SECONDS each (default 60)
#   ./ace-ci0.4.6.sh msrv      check + test with Rust 1.97 (SKIPPED when not installed)
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/tools/ace-common0.4.6.sh"
cd "$ACE_ROOT"

case "${1:-}" in
  pr)
    ./ace-build0.4.6.sh --no-demo
    python3 tools/ace-code_audit0.4.6.py
    ./ace-benchmark0.4.6.sh release-performance --quick
    ./demo/ace-run-demo0.4.6.sh
    ;;
  release)
    [[ -n "${ACE_AB_BASELINE_TREE:-}" ]] || ace_die "set ACE_AB_BASELINE_TREE to the $ACE_BASELINE source tree"
    ./ace-release0.4.6.sh "$ACE_AB_BASELINE_TREE"
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
    # Use a pinned 1.97.0 / 1.97 toolchain when installed, otherwise the default one if it is 1.97.x.
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
