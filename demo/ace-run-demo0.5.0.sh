#!/usr/bin/env bash
# ACE 0.5.0 product demo (< 1 minute after a release build). No benchmarks.
#
#   1. four representative workloads -> ratio, throughput, selected codecs, verified roundtrip
#   2. Float lane (new in 0.5.0): float / sparse-change series with the lane on and off,
#      format version, TS1 modes, the planner route of a block
#   3. random range read from an indexed file, compared with the source slice
#   4. bounded-memory streaming compression (byte-identical to in-memory compression),
#      including a TS1 file whose header is rewritten as Format 1.4
#   5. determinism: 1 thread vs all threads vs ACE_SIMD=scalar (SHA-256)
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/tools/ace-common0.5.0.sh"
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
echo "(timings include process start-up and file I/O; see ace-benchmark0.5.0.sh for measurements)"

ace_step "2. Float lane (TS1 / Format 1.4): lane on vs --disable-float, 1 thread, $SIZE each"
printf '%-24s %-9s %10s %10s %8s %12s  %s\n' workload profile 'ratio on' 'ratio off' format 'comp MB/s' 'TS1 blocks'
for spec in f64-constant:balanced f64-step:balanced f64-smooth:fast f64-noisy:fast \
            f64-sensor-temperature:balanced int-sparse-change:balanced; do
  workload="${spec%%:*}"; profile="${spec##*:}"
  src="$WORK/$workload.bin"; on="$WORK/$workload.ace"; off="$WORK/$workload-off.ace"; out="$WORK/$workload.out"
  "$CORPUS" "$workload" "$SIZE" "$src"
  t0=$(now_ns); "$ACE" compress --threads 1 --profile "$profile" "$src" "$on" >/dev/null; t1=$(now_ns)
  "$ACE" compress --threads 1 --profile "$profile" --disable-float "$src" "$off" >/dev/null
  "$ACE" decompress "$on" "$out"
  cmp -s "$src" "$out" || ace_die "$workload: roundtrip mismatch"
  bytes=$(stat -c %s "$src")
  format=$("$ACE" inspect "$on" | awk "NR == 1" | grep -o 'format 1\.[0-9]' | cut -d' ' -f2)
  modes=$("$ACE" inspect "$on" --blocks | { grep -o 'ts1 mode=[a-z_0-9]*' || true; } | sort | uniq -c \
          | awk '{sub("mode=","",$3); printf "%s x%s ", $3, $1}')
  printf '%-24s %-9s %10.2f %10.2f %8s %12s  %s\n' "$workload" "$profile" \
    "$(python3 -c "print($bytes/$(stat -c %s "$on"))")" "$(python3 -c "print($bytes/$(stat -c %s "$off"))")" \
    "$format" "$(mb_s "$bytes" $((t1 - t0)))" "${modes:--}"
done
echo "Files without a TS1 block stay Format 1.3 (readable by ACE 0.4.x); a TS1 block makes them 1.4."
echo "Planner V5 route of block 0 of f64-step (FAST):"
"$ACE" explain --profile fast "$WORK/f64-step.bin" | awk '/^block 1 /{exit} /route=|float width|ts1 selected/'

ace_step "3. random access: 64 KiB at offset 5 000 000 of structured-json"
"$ACE" read-range "$WORK/structured-json.ace" 5000000 65536 "$WORK/range.out"
python3 -c "import sys; f=open(sys.argv[1],'rb'); f.seek(5000000); open(sys.argv[2],'wb').write(f.read(65536))" \
  "$WORK/structured-json.bin" "$WORK/range.expected"
cmp -s "$WORK/range.out" "$WORK/range.expected" || ace_die "range mismatch"
echo "range read: PASS (only the intersecting blocks were decoded)"

ace_step "4. bounded-memory streaming (mixed, 64 MiB; f64-step with a Format 1.4 header rewrite)"
"$CORPUS" mixed 64M "$WORK/mixed.bin"
"$ACE" compress-stream "$WORK/mixed.bin" "$WORK/mixed-stream.ace"
"$ACE" compress --threads 1 "$WORK/mixed.bin" "$WORK/mixed-memory.ace" >/dev/null
cmp -s "$WORK/mixed-stream.ace" "$WORK/mixed-memory.ace" || ace_die "stream != in-memory"
"$ACE" verify "$WORK/mixed-stream.ace" >/dev/null
echo "streaming output is byte-identical to in-memory compression and verifies: PASS"
"$ACE" compress-stream "$WORK/f64-step.bin" "$WORK/f64-step-stream.ace"
cmp -s "$WORK/f64-step-stream.ace" "$WORK/f64-step.ace" || ace_die "TS1 stream != in-memory"
echo "TS1 stream (header rewritten as 1.4) is byte-identical to in-memory compression: PASS"

ace_step "5. determinism (SHA-256 of mixed 64 MiB, BALANCED; f64-noisy FAST)"
"$ACE" compress --threads 0 "$WORK/mixed.bin" "$WORK/mixed-all.ace" >/dev/null
ACE_SIMD=scalar "$ACE" compress --threads 1 "$WORK/mixed.bin" "$WORK/mixed-scalar.ace" >/dev/null
for variant in memory all scalar; do
  printf '  %-8s %s\n' "$variant" "$(sha "$WORK/mixed-$variant.ace")"
done
[[ "$(sha "$WORK/mixed-memory.ace")" == "$(sha "$WORK/mixed-all.ace")" &&
   "$(sha "$WORK/mixed-memory.ace")" == "$(sha "$WORK/mixed-scalar.ace")" ]] || ace_die "non-deterministic output"
echo "1 thread == all threads == scalar backend: PASS"
"$ACE" compress --threads 0 --profile fast "$WORK/f64-noisy.bin" "$WORK/noisy-all.ace" >/dev/null
ACE_SIMD=scalar "$ACE" compress --threads 1 --profile fast "$WORK/f64-noisy.bin" "$WORK/noisy-scalar.ace" >/dev/null
[[ "$(sha "$WORK/f64-noisy.ace")" == "$(sha "$WORK/noisy-all.ace")" &&
   "$(sha "$WORK/f64-noisy.ace")" == "$(sha "$WORK/noisy-scalar.ace")" ]] || ace_die "non-deterministic Float lane output"
echo "Float lane (Gorilla blocks): 1 thread == all threads == scalar backend: PASS"

rm -rf "$WORK"
ace_log "demo completed"
