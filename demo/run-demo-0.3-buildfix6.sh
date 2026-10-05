#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

mkdir -p examples/data examples/results /tmp/ace-demo-0.3-buildfix6
python3 tools/generate_corpus.py --output examples/data/mixed-demo.bin --mib 16

echo '[ACE 0.3-buildfix6] Planner V3.3 / quality-envelope explain'
cargo run --release -p ace-cli -- explain examples/data/mixed-demo.bin --profile balanced | sed -n '1,110p'

echo '[ACE 0.3-buildfix6] in-memory Format 1.2 compression'
cargo run --release -p ace-cli -- compress   examples/data/mixed-demo.bin   /tmp/ace-demo-0.3-buildfix6/mixed.ace   --profile balanced --threads 4
cargo run --release -p ace-cli -- verify /tmp/ace-demo-0.3-buildfix6/mixed.ace

echo '[ACE 0.3-buildfix6] bounded streaming compression'
cargo run --release -p ace-cli -- compress-stream   examples/data/mixed-demo.bin   /tmp/ace-demo-0.3-buildfix6/mixed-stream.ace   --profile balanced
cargo run --release -p ace-cli -- verify /tmp/ace-demo-0.3-buildfix6/mixed-stream.ace

echo '[ACE 0.3-buildfix6] planner benchmark'
./benchmark.sh planner

echo '[ACE 0.3-buildfix6] compression benchmark'
./benchmark.sh compression

echo 'Demo complete. Full release benchmark: ./benchmark.sh all'
