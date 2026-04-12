#!/usr/bin/env bash
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

publish_order=(
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
    statum-core)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

version_for_crate() {
  awk '
    $0 == "[package]" { in_package = 1; next }
    /^\[/ { in_package = 0 }
    in_package && /^version = "/ {
      gsub(/^version = "/, "", $0)
      gsub(/"$/, "", $0)
      print
      exit
    }
  ' "$1/Cargo.toml"
}

crates_io_version_exists() {
  local crate="$1"
  local version="$2"
  local status=0

  curl -fsSI "https://crates.io/api/v1/crates/$crate/$version" >/dev/null || status=$?
  case "$status" in
    0)
      return 0
      ;;
    22)
      return 1
      ;;
  esac

  if [[ $status -ne 0 ]]; then
    echo "error: failed to query crates.io for $crate $version" >&2
    exit "$status"
  fi
}

for crate in "${publish_order[@]}"; do
  version=$(version_for_crate "$crate")
  if [[ -z "$version" ]]; then
    echo "error: failed to read package version for $crate" >&2
    exit 1
  fi

  if crates_io_version_exists "$crate" "$version"; then
    echo "error: $crate is already published at version $version; bump versions before release" >&2
    exit 1
  fi

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
