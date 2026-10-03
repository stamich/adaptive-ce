#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' '=== ACE 0.2 adaptive compression demo ==='
cargo run --release -p ace-demo-0-2
printf '%s\n' '=== ACE 0.2 random-access demo ==='
cargo run --release -p ace-random-access-demo-0-2
printf '%s\n' '=== ACE 0.2 parallel determinism demo ==='
cargo run --release -p ace-parallel-demo-0-2
