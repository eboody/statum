#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/comment_graph_diff_pr.sh --baseline <snapshot.json> --current <snapshot.json> --pr <number> [--artifact <url-or-path>]

Renders a Statum graph-diff PR comment from serialized StableGraphMetadata
snapshots. The diff's authority surface is the two snapshot JSON files; this
script does not inspect Rust source, expanded macros, type-checked items, or
runtime behavior.

Required environment:
  GH_TOKEN or GITHUB_TOKEN with permission to comment on the PR.
  gh CLI authenticated for the repository.

Security note:
  This script executes repository code with `cargo run` before posting a PR
  comment. Do not run it in a privileged PR context for untrusted fork code.
USAGE
}

baseline=""
current=""
pr_number="${PR_NUMBER:-}"
artifact=""

while (($#)); do
  case "$1" in
    --baseline)
      baseline="${2:?--baseline requires a value}"
      shift 2
      ;;
    --current)
      current="${2:?--current requires a value}"
      shift 2
      ;;
    --pr)
      pr_number="${2:?--pr requires a value}"
      shift 2
      ;;
    --artifact)
      artifact="${2:?--artifact requires a value}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$baseline" || -z "$current" || -z "$pr_number" ]]; then
  usage >&2
  exit 2
fi

if [[ ! -f "$baseline" ]]; then
  echo "baseline snapshot not found: $baseline" >&2
  exit 1
fi
if [[ ! -f "$current" ]]; then
  echo "current snapshot not found: $current" >&2
  exit 1
fi
if ! command -v gh >/dev/null 2>&1; then
  echo "gh CLI is required to comment on pull requests" >&2
  exit 1
fi

mkdir -p target/statum-graph
json_report="target/statum-graph/diff.json"
markdown_report="target/statum-graph/diff.md"
comment_body="target/statum-graph/pr-comment.md"

cargo run -q -p cargo-statum -- graph diff \
  --baseline "$baseline" \
  --current "$current" \
  --format json > "$json_report"

cargo run -q -p cargo-statum -- graph diff \
  --baseline "$baseline" \
  --current "$current" \
  --format markdown > "$markdown_report"

{
  echo '<!-- statum-graph-diff -->'
  cat "$markdown_report"
  if [[ -n "$artifact" ]]; then
    echo
    echo "CI artifact: \`$artifact\`"
  else
    echo
    echo "CI artifact: \`$json_report\`"
  fi
} > "$comment_body"

gh pr comment "$pr_number" --body-file "$comment_body"

echo "commented Statum graph diff on PR #$pr_number"
echo "JSON report: $json_report"
echo "Markdown report: $markdown_report"
