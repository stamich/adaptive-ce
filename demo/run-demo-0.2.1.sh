#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
mkdir -p examples/data examples/results /tmp/ace-demo-0.2.1
python3 tools/generate_corpus.py --output examples/data/mixed-demo.bin --mib 16

echo "[ACE 0.2.1] explain BALANCED planner"
cargo run --release -p ace-cli -- explain examples/data/mixed-demo.bin --profile balanced | sed -n '1,120p'

echo "[ACE 0.2.1] compress/decompress"
cargo run --release -p ace-cli -- compress examples/data/mixed-demo.bin /tmp/ace-demo-0.2.1/mixed.ace --profile balanced --threads 4
cargo run --release -p ace-cli -- verify /tmp/ace-demo-0.2.1/mixed.ace
cargo run --release -p ace-cli -- inspect /tmp/ace-demo-0.2.1/mixed.ace --blocks | sed -n '1,40p'

echo "[ACE 0.2.1] indexed range with physical-read diagnostics"
cargo run --release -p ace-cli -- read-range /tmp/ace-demo-0.2.1/mixed.ace 3000000 65536 /tmp/ace-demo-0.2.1/range.bin

echo "[ACE 0.2.1] benchmark planner and random access"
./benchmark.sh planner
./benchmark.sh random-access

echo "Demo complete. Full release benchmark: ./benchmark.sh all"
