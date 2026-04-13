#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

fail=0

mapfile -t empty_root_dirs < <(
  find . -maxdepth 1 -mindepth 1 -type d -empty \
    ! -name '.git' \
    ! -name 'target' \
    | sort
)

if ((${#empty_root_dirs[@]} > 0)); then
  echo "workspace hygiene: unexpected empty top-level directories"
  printf '  %s\n' "${empty_root_dirs[@]}"
  fail=1
fi

mapfile -t empty_root_dotfiles < <(
  find . -maxdepth 1 -mindepth 1 -type f -name '.*' -size 0 \
    ! -name '.codex' \
    ! -name '.gitignore' \
    | sort
)

if ((${#empty_root_dotfiles[@]} > 0)); then
  echo "workspace hygiene: unexpected zero-byte top-level dotfiles"
  printf '  %s\n' "${empty_root_dotfiles[@]}"
  fail=1
fi

trybuild_wip_files=()
if [[ -d statum-macros/wip ]]; then
  mapfile -t trybuild_wip_files < <(
    find statum-macros/wip -mindepth 1 ! -name '.gitignore' | sort
  )
fi

if ((${#trybuild_wip_files[@]} > 0)); then
  echo "workspace hygiene: statum-macros/wip contains scratch outputs"
  printf '  %s\n' "${trybuild_wip_files[@]}"
  fail=1
fi

if ((fail != 0)); then
  exit 1
fi

echo "workspace hygiene ok"
