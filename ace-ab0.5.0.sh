#!/usr/bin/env bash
# Interleaved A/B performance comparison against a baseline source tree.
#
#   ./ace-ab0.5.0.sh <baseline-source-tree> [--quick] [--cases id,id]
#
# The baseline tree is an unpacked ACE 0.4.6 (or any 0.4.x / 0.5.x) source package.
# Writes examples/results/ab-0.5.0-vs-0.4.6.json; exit 0 pass, 1 fail, 3 unstable.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/tools/ace-common0.5.0.sh"
cd "$ACE_ROOT"

(( $# >= 1 )) || ace_die "usage: $0 <baseline-source-tree> [--quick] [--cases ids]"
baseline_tree="$1"; shift
[[ -f "$baseline_tree/Cargo.toml" ]] || ace_die "not a source tree: $baseline_tree"
exec python3 tools/ace-ab0.5.0.py --baseline-tree "$baseline_tree" \
  --baseline-label "$ACE_BASELINE" \
  --output "$ACE_RESULTS/ab-${ACE_VERSION}-vs-${ACE_BASELINE}.json" "$@"
