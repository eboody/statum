#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: bash scripts/benchmark_compile.sh [--iterations N] [--mode cold|warm|both]

Measures compile-check time for two local fixtures:
- benchmarks/compile/statum-fixture
- benchmarks/compile/plain-fixture

Modes:
- cold: remove the fixture target directory before every measured run
- warm: prime the target directory once, then measure incremental checks
- both: run both modes (default)
EOF
}

iterations=3
mode="both"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --iterations)
      iterations="$2"
      shift 2
      ;;
    --mode)
      mode="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if ! [[ "$iterations" =~ ^[0-9]+$ ]] || [[ "$iterations" -lt 1 ]]; then
  echo "--iterations must be a positive integer" >&2
  exit 1
fi

case "$mode" in
  cold|warm|both) ;;
  *)
    echo "--mode must be one of: cold, warm, both" >&2
    exit 1
    ;;
esac

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

measure_once() {
  local manifest_path="$1"
  local target_dir="$2"
  local label="$3"

  local start_ns end_ns elapsed_ms
  start_ns="$(date +%s%N)"
  CARGO_INCREMENTAL=0 \
  CARGO_TARGET_DIR="$target_dir" \
    cargo check --manifest-path "$manifest_path" --quiet
  end_ns="$(date +%s%N)"
  elapsed_ms="$(( (end_ns - start_ns) / 1000000 ))"
  printf '%s\n' "$elapsed_ms"
}

run_fixture_mode() {
  local fixture_name="$1"
  local manifest_path="$2"
  local mode_name="$3"
  local target_dir="$repo_root/target/compile-bench/${fixture_name}/${mode_name}"
  local total_ms=0

  if [[ "$mode_name" == "warm" ]]; then
    rm -rf "$target_dir"
    echo "Priming ${fixture_name} (${mode_name})..."
    measure_once "$manifest_path" "$target_dir" "${fixture_name}-${mode_name}-prime" >/dev/null
  fi

  for run in $(seq 1 "$iterations"); do
    if [[ "$mode_name" == "cold" ]]; then
      rm -rf "$target_dir"
    fi

    local elapsed_ms
    elapsed_ms="$(measure_once "$manifest_path" "$target_dir" "${fixture_name}-${mode_name}-${run}")"
    total_ms="$((total_ms + elapsed_ms))"
    printf '%-8s %-4s run %d: %s ms\n' "$fixture_name" "$mode_name" "$run" "$elapsed_ms"
  done

  printf '%-8s %-4s avg: %s ms\n' \
    "$fixture_name" \
    "$mode_name" \
    "$(( total_ms / iterations ))"
}

run_mode() {
  local mode_name="$1"
  run_fixture_mode "statum" "$repo_root/benchmarks/compile/statum-fixture/Cargo.toml" "$mode_name"
  run_fixture_mode "plain" "$repo_root/benchmarks/compile/plain-fixture/Cargo.toml" "$mode_name"
}

if [[ "$mode" == "cold" || "$mode" == "both" ]]; then
  run_mode "cold"
fi

if [[ "$mode" == "warm" || "$mode" == "both" ]]; then
  run_mode "warm"
fi
