#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

mkdir -p examples/data examples/results examples/corpus/numeric /tmp/ace-demo-0.4

echo '[ACE 0.4] generate deterministic numeric Corpus V3'
python3 tools/generate_numeric_corpus.py \
  --output-dir examples/corpus/numeric \
  --sizes-mib 1,16

cp examples/corpus/numeric/u32-counter-16m.bin /tmp/ace-demo-0.4/numeric.bin

echo '[ACE 0.4] workspace build/test'
cargo build --workspace --release
cargo test --workspace

echo '[ACE 0.4] Planner V4 numeric explain'
cargo run --release -p ace-cli -- explain /tmp/ace-demo-0.4/numeric.bin --profile balanced | sed -n '1,140p'

echo '[ACE 0.4] Format 1.3 numeric compression roundtrip'
cargo run --release -p ace-cli -- compress \
  /tmp/ace-demo-0.4/numeric.bin \
  /tmp/ace-demo-0.4/numeric.ace \
  --profile balanced \
  --threads 1
cargo run --release -p ace-cli -- inspect /tmp/ace-demo-0.4/numeric.ace --blocks | sed -n '1,40p'
cargo run --release -p ace-cli -- verify /tmp/ace-demo-0.4/numeric.ace

echo '[ACE 0.4] Auto block policy for sequential numeric input'
cargo run --release -p ace-cli -- compress \
  /tmp/ace-demo-0.4/numeric.bin \
  /tmp/ace-demo-0.4/numeric-auto.ace \
  --profile balanced \
  --threads 1 \
  --block-policy auto \
  --access-hint sequential
cargo run --release -p ace-cli -- inspect /tmp/ace-demo-0.4/numeric-auto.ace | sed -n '1,10p'

echo '[ACE 0.4] deterministic output smoke'
cargo run --release -p ace-cli -- compress /tmp/ace-demo-0.4/numeric.bin /tmp/ace-demo-0.4/numeric-1t.ace --profile balanced --threads 1
cargo run --release -p ace-cli -- compress /tmp/ace-demo-0.4/numeric.bin /tmp/ace-demo-0.4/numeric-4t.ace --profile balanced --threads 4
cmp /tmp/ace-demo-0.4/numeric-1t.ace /tmp/ace-demo-0.4/numeric-4t.ace
echo 'determinism: PASS'

echo '[ACE 0.4] numeric benchmark'
./benchmark.sh numeric

echo '[ACE 0.4] numeric ablation'
./benchmark.sh numeric-ablation

echo '[ACE 0.4] block policy comparison'
./benchmark.sh block-policy

echo '[ACE 0.4] full release contract'
echo './benchmark.sh all'
