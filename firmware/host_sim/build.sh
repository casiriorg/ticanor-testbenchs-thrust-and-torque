#!/usr/bin/env bash
# Builds the firmware as a PC program wired to a simulated bench.
#
#   firmware/host_sim/build.sh [output-binary]
#
# The desktop app's integration test (app/crates/tb-core/tests/firmware_sim.rs)
# runs this binary when the TB_HOST_SIM environment variable points to it.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="${1:-$HERE/../build/host/thrustbench_host_sim}"
mkdir -p "$(dirname "$OUT")"

mapfile -t SOURCES < <(find "$HERE/../ThrustBench/src" -name '*.cpp' | sort)

g++ -std=c++17 -O1 -Wall -Wextra -Wno-unused-parameter \
  -I"$HERE/include" \
  "${SOURCES[@]}" "$HERE/HostBoard.cpp" "$HERE/main.cpp" \
  -o "$OUT" -lpthread

echo "Built $OUT"
