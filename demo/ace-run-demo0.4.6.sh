#!/usr/bin/env bash
# ACE 0.4.6 product demo (< 1 minute after a release build). No benchmarks.
#
#   1. four representative workloads -> ratio, throughput, selected codecs, verified roundtrip
#   2. random range read from an indexed file, compared with the source slice
#   3. bounded-memory streaming compression (byte-identical to in-memory compression)
#   4. determinism: 1 thread vs all threads vs ACE_SIMD=scalar (SHA-256)
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/tools/ace-common0.4.6.sh"
cd "$ACE_ROOT"

WORK="$ACE_ROOT/target/demo-$ACE_VERSION"
SIZE="16M"
rm -rf "$WORK" && mkdir -p "$WORK"

ace_step "build (release): ace CLI + ace-corpus generator"
cargo build --release --quiet -p ace-cli -p ace-corpus
ACE="$(ace_bin ace)"
CORPUS="$(ace_bin ace-corpus)"

now_ns() { date +%s%N; }
mb_s() { python3 -c "print(f'{$1 / 1048576 / ($2 / 1e9):8.1f}')"; }
sha() { sha256sum "$1" | cut -d' ' -f1; }

ace_step "1. workloads (BALANCED, 1 thread, $SIZE each)"
printf '%-20s %8s %12s %12s  %s\n' workload ratio 'comp MB/s' 'dec MB/s' 'codecs (blocks)'
for workload in structured-json u32-counter u64-timestamps-ns random; do
  src="$WORK/$workload.bin"; ace="$WORK/$workload.ace"; out="$WORK/$workload.out"
  "$CORPUS" "$workload" "$SIZE" "$src"
  t0=$(now_ns); "$ACE" compress --threads 1 "$src" "$ace" >/dev/null; t1=$(now_ns)
  "$ACE" decompress "$ace" "$out"; t2=$(now_ns)
  cmp -s "$src" "$out" || ace_die "$workload: roundtrip mismatch"
  bytes=$(stat -c %s "$src"); packed=$(stat -c %s "$ace")
  codecs=$("$ACE" inspect "$ace" --blocks | grep -o 'codec=[A-Za-z0-9]*' | sort | uniq -c \
           | awk '{sub("codec=","",$2); printf "%s x%s ", $2, $1}')
  printf '%-20s %8.2f %12s %12s  %s\n' "$workload" "$(python3 -c "print($bytes/$packed)")" \
    "$(mb_s "$bytes" $((t1 - t0)))" "$(mb_s "$bytes" $((t2 - t1)))" "$codecs"
done
echo "(timings include process start-up and file I/O; see ace-benchmark0.4.6.sh for measurements)"

ace_step "2. random access: 64 KiB at offset 5 000 000 of structured-json"
"$ACE" read-range "$WORK/structured-json.ace" 5000000 65536 "$WORK/range.out"
python3 -c "import sys; f=open(sys.argv[1],'rb'); f.seek(5000000); open(sys.argv[2],'wb').write(f.read(65536))" \
  "$WORK/structured-json.bin" "$WORK/range.expected"
cmp -s "$WORK/range.out" "$WORK/range.expected" || ace_die "range mismatch"
echo "range read: PASS (only the intersecting blocks were decoded)"

ace_step "3. bounded-memory streaming (mixed, 64 MiB)"
"$CORPUS" mixed 64M "$WORK/mixed.bin"
"$ACE" compress-stream "$WORK/mixed.bin" "$WORK/mixed-stream.ace"
"$ACE" compress --threads 1 "$WORK/mixed.bin" "$WORK/mixed-memory.ace" >/dev/null
cmp -s "$WORK/mixed-stream.ace" "$WORK/mixed-memory.ace" || ace_die "stream != in-memory"
"$ACE" verify "$WORK/mixed-stream.ace" >/dev/null
echo "streaming output is byte-identical to in-memory compression and verifies: PASS"

ace_step "4. determinism (SHA-256 of mixed 64 MiB, BALANCED)"
"$ACE" compress --threads 0 "$WORK/mixed.bin" "$WORK/mixed-all.ace" >/dev/null
ACE_SIMD=scalar "$ACE" compress --threads 1 "$WORK/mixed.bin" "$WORK/mixed-scalar.ace" >/dev/null
for variant in memory all scalar; do
  printf '  %-8s %s\n' "$variant" "$(sha "$WORK/mixed-$variant.ace")"
done
[[ "$(sha "$WORK/mixed-memory.ace")" == "$(sha "$WORK/mixed-all.ace")" &&
   "$(sha "$WORK/mixed-memory.ace")" == "$(sha "$WORK/mixed-scalar.ace")" ]] || ace_die "non-deterministic output"
echo "1 thread == all threads == scalar backend: PASS"

rm -rf "$WORK"
ace_log "demo completed"
