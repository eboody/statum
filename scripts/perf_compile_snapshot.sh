#!/usr/bin/env bash
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

ms_now() {
  date +%s%3N
}

run_and_time() {
  local label="$1"
  shift
  local start end
  start=$(ms_now)
  "$@"
  end=$(ms_now)
  echo "$label=$((end - start))"
}

echo "== Statum compile-time snapshot =="
echo "repo=$repo_root"
echo "rustc=$(rustc --version)"

printf '\n[1/3] cleaning workspace\n'
run_and_time clean_ms cargo clean >/dev/null

echo "[2/3] workspace check with cargo timings"
check_line=$(run_and_time check_workspace_ms cargo check --workspace --offline --timings)
echo "$check_line"

echo "[3/3] statum-macros tests"
test_line=$(run_and_time test_statum_macros_ms cargo test -p statum-macros --offline)
echo "$test_line"

timing_html=$(ls -1t target/cargo-timings/cargo-timing-*.html 2>/dev/null | head -n1 || true)
if [[ -n "$timing_html" ]]; then
  echo "timings_report=$timing_html"
else
  echo "timings_report=missing"
fi

printf '\n== Summary ==\n'
echo "$check_line"
echo "$test_line"
