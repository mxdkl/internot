#!/usr/bin/env bash
# Local CI: the workspace tests, then the performance gate on a release build.
# Exits non-zero if either fails. Extra arguments go to perf-gate, e.g.
#   perf/check.sh --quick
#   perf/check.sh --suite core_hash
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== cargo test --workspace"
cargo test --workspace

echo
echo "== perf-gate $*"
cargo run --release -p internot_perf --bin perf-gate -- "$@"
