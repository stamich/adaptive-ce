#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

echo '[ACE 0.4-buildfix4] cargo check'
cargo check --workspace

echo '[ACE 0.4-buildfix4] cargo test'
cargo test --workspace

echo '[ACE 0.4-buildfix4] release build'
cargo build --workspace --release

echo '[ACE 0.4-buildfix4] demo'
./demo/ace-run-demo0.4-buildfix4.sh

echo '[ACE 0.4-buildfix4] full benchmark/release contract'
./ace-benchmark0.4-buildfix4.sh all

echo '[ACE 0.4-buildfix4] build pipeline completed successfully'
