#!/bin/bash
# Interleaved A/B timing of one source file's two versions.
#
#   perf/ab.sh <crate> <example> <file> <A version of file> [runs] [-- args]
#
# Builds <example> of <crate> twice: once with <file> as it is (B) and once
# with the A version copied over it. It restores <file> afterwards, even on
# failure. Then it runs A and B alternately, waiting for the CPU to cool to
# 60 °C before each run. Alternating cancels drift in temperature and
# background load. Separate gate runs of a DRAM-bound lookup vary ~10%, too
# much to see a 5% change.
#
# Example (lookup_timing prints a checksum, which must match between A and B
# when the change should not alter any answer):
#   cp internot_society/src/world.rs /tmp/world_before.rs   # before editing
#   ...edit...
#   perf/ab.sh internot_society lookup_timing internot_society/src/world.rs /tmp/world_before.rs 3
set -euo pipefail
crate=$1; example=$2; file=$3; a_file=$4; runs=${5:-3}
shift $(( $# >= 5 ? 5 : 4 )); [ "${1:-}" = "--" ] && shift
cd "$(dirname "$0")/.."
work=$(mktemp -d)
cp "$file" "$work/b_source"
trap 'cp "$work/b_source" "$file"; rm -rf "$work"' EXIT
build() { cargo build --release -q -p "$crate" --example "$example" && cp "target/release/examples/$example" "$work/$1"; }
build b
cp "$a_file" "$file"
build a
cp "$work/b_source" "$file"
sensor=$(grep -l -E '^(k10temp|coretemp)$' /sys/class/hwmon/hwmon*/name 2>/dev/null | head -1 | xargs -r dirname)
cool() { [ -n "$sensor" ] || return 0; while [ "$(cat "$sensor/temp1_input")" -gt 60000 ]; do sleep 0.5; done; }
for _ in $(seq "$runs"); do
    cool; echo "A: $("$work/a" "$@")"
    cool; echo "B: $("$work/b" "$@")"
done
