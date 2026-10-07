#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

echo '[ACE 0.4.5-buildfix2] cargo check'
cargo check --workspace

echo '[ACE 0.4.5-buildfix2] cargo test'
cargo test --workspace

echo '[ACE 0.4.5-buildfix2] release build'
cargo build --workspace --release

echo '[ACE 0.4.5-buildfix2] demo'
./demo/ace-run-demo0.4.5-buildfix2.sh

echo '[ACE 0.4.5-buildfix2] full benchmark/release contract'
./ace-benchmark0.4.5-buildfix2.sh all

echo '[ACE 0.4.5-buildfix2] build pipeline completed successfully'
