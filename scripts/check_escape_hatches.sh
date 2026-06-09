#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
cd "$repo_root"

status=0

require_text() {
  local file=$1
  local needle=$2

  if [[ ! -f "$file" ]]; then
    printf 'missing escape-hatch doc: %s\n' "$file"
    status=1
    return
  fi

  if ! grep -Fq -- "$needle" "$file"; then
    printf '%s: missing required escape-hatch text: %s\n' "$file" "$needle"
    status=1
  fi
}

require_text docs/escape-hatches.md '#[introspect(return = ...)]'
require_text docs/escape-hatches.md 'No current public API named'
require_text docs/escape-hatches.md 'assume_state'
require_text docs/escape-hatches.md 'from_parts_unchecked'
require_text docs/escape-hatches.md 'unchecked'
require_text docs/rehydration-vocabulary.md 'assume_state'
require_text docs/rehydration-vocabulary.md 'from_parts_unchecked'
require_text docs/persistence-and-validators.md 'escape-hatches.md'

if grep -RInE '\b(pub[[:space:]]+(unsafe[[:space:]]+)?fn|fn)[[:space:]]+([[:alnum:]_]*unchecked[[:alnum:]_]*|assume_state|from_parts_unchecked)\b' \
  --include='*.rs' \
  statum statum-core statum-macros statum-examples; then
  printf 'new typed-rehydration escape hatch found; update docs/escape-hatches.md and this audit allowlist\n'
  status=1
fi

exit "$status"
