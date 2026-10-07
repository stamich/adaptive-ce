#!/usr/bin/env bash
# Compare ACE benchmark results of any two milestones (schema 2.0 / 2.1).
#
#   ./ace-benchmark-compare0.5.0.sh OLD.json NEW.json
#   ./ace-benchmark-compare0.5.0.sh OLD_DIR NEW_DIR     every family present in both
#
# Cross-machine or cross-methodology deltas are diagnostics only; release decisions use
# the interleaved A/B (ace-ab0.5.0.sh).
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/tools/ace-common0.5.0.sh"

(( $# == 2 )) || ace_die "usage: $0 OLD NEW (files or result directories)"
compare=(python3 "$ACE_ROOT/tools/ace-benchmark_compare0.5.0.py")

if [[ -f "$1" && -f "$2" ]]; then
  "${compare[@]}" "$1" "$2"
  exit 0
fi
[[ -d "$1" && -d "$2" ]] || ace_die "arguments must be two files or two directories"
for new in "$2"/benchmark-*-*.json; do
  family="$(basename "$new" .json | sed -E 's/^benchmark-[0-9][^-]*(-buildfix[0-9]+|-compilefix)*-//')"
  old="$(ls "$1"/benchmark-*-"$family".json 2>/dev/null | head -n1 || true)"
  [[ -n "$old" ]] || continue
  printf '\n=== %s ===\n' "$family"
  "${compare[@]}" "$old" "$new"
done
