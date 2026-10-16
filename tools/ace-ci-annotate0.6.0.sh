#!/usr/bin/env bash
# Re-emits the failure lines of a CI log as GitHub `::error::` annotations (at most 40), so a
# failed job explains itself in the check-run summary.
#
#   tools/ace-ci-annotate0.6.0.sh LOG
set -euo pipefail
log="${1:?usage: $0 LOG}"
[[ -f "$log" ]] || exit 0
grep -E -A3 '^(error|warning: unused)|panicked at|^test .* FAILED$|^---- .* ----$|FAIL|ERROR' "$log" \
  | grep -v '^--$' | head -n 40 | while IFS= read -r line; do
    # Annotation messages are single lines; `%` must be escaped first.
    line="${line//'%'/'%25'}"
    printf '::error::%s\n' "$line"
  done
