#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

run() {
  printf '\n==> %s\n' "$*"
  "$@"
}

run_env() {
  printf '\n==> %s %s\n' "$1" "${*:2}"
  env "$@"
}

run_optional_audit_scripts() {
  local pattern=$1
  local script

  # The closeout gate is the repo's single CI-parity entrypoint. These globs let
  # new diagnostics or graph audit scripts join that entrypoint without editing
  # GitHub Actions again; known base gates are run explicitly below and skipped
  # here to avoid duplicate execution.
  shopt -s nullglob
  local scripts=(scripts/$pattern)
  shopt -u nullglob

  for script in "${scripts[@]}"; do
    case "$script" in
      scripts/check_ci_parity.sh|\
        scripts/check_diagnostics_coverage.sh|\
        scripts/check_escape_hatches.sh|\
        scripts/check_readme_links.sh|\
        scripts/check_workspace_hygiene.sh)
        continue
        ;;
    esac

    run bash "$script"
  done
}

run cargo modum check --root . --mode warn
run cargo fmt --all --check
run bash scripts/check_readme_links.sh
run bash scripts/check_escape_hatches.sh
run bash scripts/check_diagnostics_coverage.sh
run_optional_audit_scripts 'check_*diagnostic*.sh'
run_optional_audit_scripts 'check_*graph*.sh'
run cargo clippy --workspace --all-targets --all-features -- -D warnings

# trybuild suites are feature-gated; run both feature selections so UI fixture
# registration, .stderr expectations, and strict-introspection-only diagnostics
# are checked before a task is closed.
run cargo test -p statum-macros
run cargo test -p statum-macros --features strict-introspection

run cargo test --workspace --all-features
run bash scripts/check_workspace_hygiene.sh
run_env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

printf '\nCI-parity closeout gates passed.\n'
