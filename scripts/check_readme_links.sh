#!/usr/bin/env bash
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

docs_files=(
  "README.md"
  "statum/README.md"
  "statum-core/README.md"
  "statum-macros/README.md"
  "macro_registry/README.md"
  "module_path_extractor/README.md"
  "statum-examples/README.md"
)

while IFS= read -r doc; do
  docs_files+=("$doc")
done < <(find docs -type f -name '*.md' | sort)

status=0

for doc in "${docs_files[@]}"; do
  if [[ ! -f "$doc" ]]; then
    echo "missing doc: $doc"
    status=1
    continue
  fi

  base_dir=$(dirname "$doc")

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
      echo "$doc: broken relative link -> $raw_link"
      status=1
    fi
  done < <(
    rg -o '\[[^\]]+\]\(([^)]+)\)' "$doc" \
      | sed -E 's/^.*\(([^)]+)\)$/\1/'
  )
done

exit "$status"
