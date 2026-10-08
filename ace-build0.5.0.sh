#!/usr/bin/env bash
# ACE 0.5.0 build pipeline: quality gates, tests, release build, docs and the demo.
# Benchmarks are deliberately NOT part of the build (see ace-benchmark0.5.0.sh).
#
#   ./ace-build0.5.0.sh            full pipeline
#   ./ace-build0.5.0.sh --no-demo  skip the demo (CI)
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/tools/ace-common0.5.0.sh"
cd "$ACE_ROOT"

run_demo=1
[[ "${1:-}" == "--no-demo" ]] && run_demo=0

ace_step "format check"
cargo fmt --all -- --check

ace_step "clippy (-D warnings, all targets)"
cargo clippy --workspace --all-targets -- -D warnings

ace_step "check"
cargo check --workspace --all-targets

ace_step "tests (debug)"
cargo test --workspace

ace_step "release build"
cargo build --workspace --release

ace_step "rustdoc (-D warnings)"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

if (( run_demo )); then
  ace_step "demo"
  ./demo/ace-run-demo0.5.0.sh
fi

ace_log "build pipeline completed successfully"
