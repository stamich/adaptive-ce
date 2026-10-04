#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p examples/data examples/results /tmp/ace-demo-0.3
python3 tools/generate_corpus.py --output examples/data/mixed-demo.bin --mib 16

echo '[ACE 0.3] Planner V3 / explain'
cargo run --release -p ace-cli -- explain examples/data/mixed-demo.bin --profile balanced | sed -n '1,90p'

echo '[ACE 0.3] in-memory Format 1.2 compression'
cargo run --release -p ace-cli -- compress examples/data/mixed-demo.bin /tmp/ace-demo-0.3/mixed.ace --profile balanced --threads 4
cargo run --release -p ace-cli -- verify /tmp/ace-demo-0.3/mixed.ace
cargo run --release -p ace-cli -- inspect /tmp/ace-demo-0.3/mixed.ace --blocks | sed -n '1,25p'

echo '[ACE 0.3] bounded streaming compression'
cargo run --release -p ace-cli -- compress-stream examples/data/mixed-demo.bin /tmp/ace-demo-0.3/mixed-stream.ace --profile balanced
cargo run --release -p ace-cli -- verify /tmp/ace-demo-0.3/mixed-stream.ace

echo '[ACE 0.3] warm indexed range'
cargo run --release -p ace-cli -- read-range /tmp/ace-demo-0.3/mixed.ace 3000000 65536 /tmp/ace-demo-0.3/range.bin

echo '[ACE 0.3] planner benchmark'
./benchmark.sh planner

echo '[ACE 0.3] streaming benchmark'
./benchmark.sh streaming

echo 'Demo complete. Full release benchmark: ./benchmark.sh all'
