#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

WORK=/tmp/ace-demo-0.4.5-buildfix2
rm -rf "$WORK"
mkdir -p "$WORK" examples/results examples/corpus/numeric

echo '[ACE 0.4.5-buildfix2] generate deterministic Numeric Corpus V3'
python3 tools/ace-generate_numeric_corpus0.4.5-buildfix2.py \
  --output-dir examples/corpus/numeric \
  --sizes-mib 1,16

cp examples/corpus/numeric/u32-counter-16m.bin "$WORK/u32-counter.bin"

echo '[ACE 0.4.5-buildfix2] build and test workspace'
cargo check --workspace
cargo test --workspace
cargo build --workspace --release

echo '[ACE 0.4.5-buildfix2] policy oracle V2'
./ace-benchmark0.4.5-buildfix2.sh policy-oracle-v2

echo '[ACE 0.4.5-buildfix2] NumericGeneral reduced search'
./ace-benchmark0.4.5-buildfix2.sh numeric-general

echo '[ACE 0.4.5-buildfix2] planner hot-path timing'
./ace-benchmark0.4.5-buildfix2.sh planner-hotpath

echo '[ACE 0.4.5-buildfix2] random-access plan diff'
./ace-benchmark0.4.5-buildfix2.sh random-access-plan-diff

echo '[ACE 0.4.5-buildfix2] NumericFast preservation'
./ace-benchmark0.4.5-buildfix2.sh numeric-fastpath

echo '[ACE 0.4.5-buildfix2] Planner V4.3 route classification'
./ace-benchmark0.4.5-buildfix2.sh planner-route

echo '[ACE 0.4.5-buildfix2] explain strong numeric workload'
cargo run --release -p ace-cli -- explain \
  "$WORK/u32-counter.bin" --profile balanced | sed -n '1,160p'

echo '[ACE 0.4.5-buildfix2] Format 1.3 roundtrip'
cargo run --release -p ace-cli -- compress \
  "$WORK/u32-counter.bin" "$WORK/u32-counter.ace" \
  --profile balanced --threads 1
cargo run --release -p ace-cli -- inspect \
  "$WORK/u32-counter.ace" --blocks | sed -n '1,60p'
cargo run --release -p ace-cli -- verify "$WORK/u32-counter.ace"

echo '[ACE 0.4.5-buildfix2] deterministic 1T/4T output'
cargo run --release -p ace-cli -- compress \
  "$WORK/u32-counter.bin" "$WORK/u32-1t.ace" \
  --profile balanced --threads 1
cargo run --release -p ace-cli -- compress \
  "$WORK/u32-counter.bin" "$WORK/u32-4t.ace" \
  --profile balanced --threads 4
cmp "$WORK/u32-1t.ace" "$WORK/u32-4t.ace"
echo 'determinism: PASS'

echo '[ACE 0.4.5-buildfix2] focused planner + numeric benchmarks'
./ace-benchmark0.4.5-buildfix2.sh planner
./ace-benchmark0.4.5-buildfix2.sh numeric
./ace-benchmark0.4.5-buildfix2.sh numeric-ablation

echo '[ACE 0.4.5-buildfix2] full contract command'
echo './ace-benchmark0.4.5-buildfix2.sh all'
