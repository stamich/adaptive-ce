#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

mkdir -p examples/data examples/results /tmp/ace-demo-0.3.1

echo '[ACE 0.3.1] generate deterministic hardening corpus'
python3 tools/generate_hardening_corpus.py \
  --output-dir examples/corpus/generated \
  --sizes-mib 1,16

python3 tools/generate_corpus.py --output examples/data/mixed-demo.bin --mib 16

echo '[ACE 0.3.1] workspace build and hardened test suites'
cargo build --workspace --release
cargo test --workspace

echo '[ACE 0.3.1] Planner V3.6 is frozen; show FAST and BALANCED explanations'
cargo run --release -p ace-cli -- explain examples/data/mixed-demo.bin --profile fast | sed -n '1,90p'
cargo run --release -p ace-cli -- explain examples/data/mixed-demo.bin --profile balanced | sed -n '1,110p'

echo '[ACE 0.3.1] deterministic output across worker counts'
cargo run --release -p ace-cli -- compress \
  examples/data/mixed-demo.bin /tmp/ace-demo-0.3.1/one-thread.ace \
  --profile balanced --threads 1
cargo run --release -p ace-cli -- compress \
  examples/data/mixed-demo.bin /tmp/ace-demo-0.3.1/four-thread.ace \
  --profile balanced --threads 4
cmp /tmp/ace-demo-0.3.1/one-thread.ace /tmp/ace-demo-0.3.1/four-thread.ace
echo 'determinism: PASS'

echo '[ACE 0.3.1] verify Format 1.2 roundtrip'
cargo run --release -p ace-cli -- verify /tmp/ace-demo-0.3.1/four-thread.ace

echo '[ACE 0.3.1] corruption rejection smoke test'
cp /tmp/ace-demo-0.3.1/four-thread.ace /tmp/ace-demo-0.3.1/corrupt.ace
python3 - <<'PY'
from pathlib import Path
path = Path("/tmp/ace-demo-0.3.1/corrupt.ace")
data = bytearray(path.read_bytes())
if len(data) > 96:
    data[80] ^= 0x5A
path.write_bytes(data)
PY
if cargo run --release -p ace-cli -- verify /tmp/ace-demo-0.3.1/corrupt.ace >/dev/null 2>&1; then
  echo 'corruption rejection: FAIL'
  exit 1
else
  echo 'corruption rejection: PASS'
fi

echo '[ACE 0.3.1] focused hardening benchmarks'
./benchmark.sh stability
./benchmark.sh block-matrix
./benchmark.sh random-access-extended
./benchmark.sh corpus

echo '[ACE 0.3.1] full release contract'
echo './benchmark.sh all'
