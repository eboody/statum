#!/usr/bin/env bash
set -euo pipefail

readmes=(
  "statum/README.md"
  "statum-core/README.md"
  "statum-macros/README.md"
  "macro_registry/README.md"
  "module_path_extractor/README.md"
  "statum-examples/README.md"
)

status=0

for readme in "${readmes[@]}"; do
  if [[ ! -f "$readme" ]]; then
    echo "missing README: $readme"
    status=1
    continue
  fi

  base_dir=$(dirname "$readme")

  while IFS= read -r raw_link; do
    link="$raw_link"
    case "$link" in
      ""|http://*|https://*|mailto:*|\#*)
        continue
        ;;
    esac

    link="${link%%#*}"
    link="${link%%\?*}"

    [[ -z "$link" ]] && continue

    if [[ "$link" = /* ]]; then
      target="$link"
    else
      target="$base_dir/$link"
    fi

    if [[ ! -e "$target" ]]; then
      echo "$readme: broken relative link -> $raw_link"
      status=1
    fi
  done < <(
    rg -o '\[[^\]]+\]\(([^)]+)\)' "$readme" \
      | sed -E 's/^.*\(([^)]+)\)$/\1/'
  )

done

exit "$status"
