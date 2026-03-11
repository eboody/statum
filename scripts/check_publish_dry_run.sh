#!/usr/bin/env bash
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

publish_order=(
  "module_path_extractor"
  "macro_registry"
  "statum-core"
  "statum-macros"
  "statum"
)

extra_args=()
if [[ "${STATUM_ALLOW_DIRTY:-0}" == "1" ]]; then
  extra_args+=(--allow-dirty)
fi

can_publish_dry_run() {
  case "$1" in
    module_path_extractor|statum-core)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

for crate in "${publish_order[@]}"; do
  if can_publish_dry_run "$crate"; then
    echo "Dry-run publishing $crate..."

    set +e
    output=$(cargo publish -p "$crate" --dry-run "${extra_args[@]}" 2>&1)
    status=$?
    set -e

    printf '%s\n' "$output"

    if [[ $status -ne 0 ]]; then
      exit "$status"
    fi

    if [[ "$output" == *"already exists on crates.io index"* ]]; then
      echo "error: $crate is already published at this version; bump versions before release" >&2
      exit 1
    fi
  else
    echo "Inspecting package contents for $crate..."
    cargo package -p "$crate" "${extra_args[@]}" --list
  fi
done
