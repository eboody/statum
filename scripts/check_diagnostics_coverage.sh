#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
cd "$repo_root"

python3 - <<'PY'
from __future__ import annotations

import re
import sys
from pathlib import Path

root = Path('.')
ui_dir = root / 'statum-macros' / 'tests' / 'ui'
macro_errors = root / 'statum-macros' / 'tests' / 'macro_errors.rs'
diag_dir = root / 'docs' / 'diagnostics'
diag_index = diag_dir / 'README.md'

errors: list[str] = []

def err(message: str) -> None:
    errors.append(message)

stderr_paths = sorted(ui_dir.glob('*.stderr'))
stderr_stems = {path.stem for path in stderr_paths}

if not stderr_paths:
    err('no trybuild .stderr fixtures found under statum-macros/tests/ui')

for stderr_path in stderr_paths:
    rs_path = stderr_path.with_suffix('.rs')
    if not rs_path.exists():
        err(f'{stderr_path}: missing source fixture {rs_path}')

macro_text = macro_errors.read_text(encoding='utf-8')
registered_compile_fail = {
    Path(match).stem
    for match in re.findall(r'\.compile_fail\("tests/ui/([^"\n]+\.rs)"\)', macro_text)
}

missing_registered_stderr = sorted(registered_compile_fail - stderr_stems)
for stem in missing_registered_stderr:
    err(f'macro_errors.rs registers compile-fail fixture without .stderr: {stem}')

unregistered_stderr = sorted(stderr_stems - registered_compile_fail)
for stem in unregistered_stderr:
    err(f'committed .stderr fixture is not registered in macro_errors.rs: {stem}')

if not diag_index.exists():
    err('missing docs/diagnostics/README.md')
    diag_text = ''
else:
    diag_text = diag_index.read_text(encoding='utf-8')

page_paths = sorted(path for path in diag_dir.glob('*.md') if path.name != 'README.md')
page_stems = {path.stem for path in page_paths}

missing_pages = sorted(stderr_stems - page_stems)
for stem in missing_pages:
    err(f'diagnostic fixture has no docs page: docs/diagnostics/{stem}.md')

orphan_pages = sorted(page_stems - stderr_stems)
for stem in orphan_pages:
    err(f'diagnostic docs page has no matching .stderr fixture: docs/diagnostics/{stem}.md')

for stem in sorted(stderr_stems):
    if f'({stem}.md)' not in diag_text:
        err(f'docs/diagnostics/README.md does not link {stem}.md')

index_entries = re.findall(
    r'^- \[([^\]]+)\]\(([^)]+\.md)\) — (diagnostic|placeholder)$',
    diag_text,
    flags=re.MULTILINE,
)
entry_stems = {name for name, _href, _kind in index_entries}
missing_entries = sorted(stderr_stems - entry_stems)
for stem in missing_entries:
    err(f'docs/diagnostics/README.md has no status entry for {stem}')

for name, href, _kind in index_entries:
    if href != f'{name}.md':
        err(f'docs/diagnostics/README.md entry for {name} links {href}, expected {name}.md')

placeholder_count = sum(1 for _name, _href, kind in index_entries if kind == 'placeholder')
diagnostic_count = sum(1 for _name, _href, kind in index_entries if kind == 'diagnostic')
expected_summary = (
    f'Known fixture count: {len(stderr_stems)}. '
    f'First-party diagnostic pages: {diagnostic_count}. '
    f'Compiler-fallback placeholders: {placeholder_count}.'
)
if diag_text and expected_summary not in diag_text:
    err(
        'docs/diagnostics/README.md summary count is stale; expected line: '
        + expected_summary
    )

if errors:
    print('diagnostics coverage check failed:', file=sys.stderr)
    for message in errors:
        print(f'- {message}', file=sys.stderr)
    sys.exit(1)

print(
    'diagnostics coverage ok: '
    f'{len(stderr_stems)} committed .stderr fixtures, '
    f'{len(registered_compile_fail)} registered compile-fail fixtures, '
    f'{diagnostic_count} diagnostic pages, '
    f'{placeholder_count} compiler-fallback placeholders'
)
PY
