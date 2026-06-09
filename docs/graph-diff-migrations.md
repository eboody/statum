# Graph Diffing For Workflow Migrations

This design defines how Statum tooling should compare two workflow graph snapshots
and report state, transition, and migration-review changes.

The goal is not to prove that two arbitrary Rust programs are behaviorally
equivalent. The diff observes two exported `StableGraphMetadata` documents and
reports changes in the workflow graph those documents describe.

## Authority Boundary

Claimed authority surface: differences between two exported Statum stable graph
metadata snapshots for one logical machine.

Actual observation point: serialized `StableGraphMetadata` documents, whose own
`authority` field currently records `cfg_pruned_macro_input`.

Unsupported cases that remain outside the diff:

- runtime-only transition choices that are not represented in the static graph
- cfg-ambiguous aliases or unsupported nested cfg return-shape ambiguity
- custom decision enums, macro-generated items, and `include!`-generated items
  rejected before stable metadata emission
- arbitrary function-body behavior
- type-checked Rust semantics outside Statum's macro-observed input
- field-level migration safety, because `StableGraphMetadata` v1 reserves field
  arrays but does not populate field metadata

Because the diff starts from exported metadata, it must not claim to be an
exhaustive semantic Rust migration analyzer. It is a workflow-graph review aid.

## Snapshot Format

CI and release automation should store one JSON snapshot per machine. The file is
a small wrapper around `StableGraphMetadata` so future tooling can compare the
same graph across crates, feature sets, or release channels without relying on
file paths alone.

```json
{
  "snapshot_version": "v1",
  "package": "statum-examples",
  "machine_key": "showcases::axum_sqlite_review::DocumentMachine",
  "feature_set": {
    "cargo_features": [],
    "target": "x86_64-unknown-linux-gnu"
  },
  "generated_by": {
    "tool": "cargo-statum",
    "command": "cargo statum graph --machine axum-sqlite-review --format json",
    "statum_version": "0.1.0"
  },
  "graph": {
    "version": "v1",
    "authority": "cfg_pruned_macro_input",
    "unsupported_cases": [
      "runtime_only_transitions",
      "cfg_ambiguous_aliases",
      "unexpanded_custom_decision_enums",
      "macro_generated_items",
      "include_generated_items",
      "field_level_presentation_metadata"
    ],
    "machine": {
      "module_path": "statum_examples::showcases::axum_sqlite_review",
      "rust_type_path": "showcases::axum_sqlite_review::DocumentMachine",
      "label": null,
      "description": null,
      "fields": []
    },
    "states": [
      {
        "rust_name": "Draft",
        "label": null,
        "description": null,
        "has_data": true,
        "fields": []
      }
    ],
    "transitions": [
      {
        "method_name": "submit",
        "label": null,
        "description": null,
        "from_state": "Draft",
        "to_states": ["InReview"]
      }
    ]
  }
}
```

Snapshot identity fields:

- `snapshot_version`: wrapper schema version, independent of
  `graph.version`.
- `package`: Cargo package that produced the machine snapshot.
- `machine_key`: stable machine selector used by tooling and PR comments.
- `feature_set`: selected feature and target context. A diff is only meaningful
  when both sides use the same intended feature set.
- `generated_by`: reproducibility metadata for humans. It is not part of the
  graph identity.
- `graph`: the `StableGraphMetadata` payload.

The diff key is `(package, machine_key, feature_set)`. If either side changes
one of those fields, the report should mark the snapshot as unmatched instead of
silently diffing unrelated graphs.

## Canonical Comparison Keys

Use stable strings from the metadata as comparison keys:

- State key: `state.rust_name`
- Transition site key: `(from_state, method_name)`
- Transition edge key: `(from_state, method_name, to_state)`

Labels and descriptions are presentation changes. They should be reported, but
not treated as workflow legality changes.

If two transitions share the same `(from_state, method_name)` within one
snapshot, the diff should fail closed with an invalid-snapshot error. Statum's
generated graph is expected to keep transition sites unique; a duplicate would
make PR comments ambiguous.

## Diff Algorithm

For each matched snapshot pair:

1. Validate wrapper versions and graph versions.
2. Validate matching `package`, `machine_key`, and intended `feature_set`.
3. Index states by `rust_name`.
4. Index transition sites by `(from_state, method_name)`.
5. Expand transition targets into edge keys `(from_state, method_name, to_state)`.
6. Compute added, removed, and shared keys for states, transition sites, and
   transition edges.
7. For shared states and transition sites, compare metadata fields that exist in
   `StableGraphMetadata` v1:
   - label
   - description
   - `has_data` for states
8. Derive migration concerns from the structural diff.
9. Render deterministic machine-readable JSON and a compact Markdown summary.

The output order should be deterministic:

1. machine key
2. severity: breaking, review, informational
3. category: states, transitions, edges, presentation, authority
4. lexical key order

## Diff Output Shape

The machine-readable report should use a stable schema so CI, release tooling,
and PR bots can consume it.

```json
{
  "diff_version": "v1",
  "machine_key": "showcases::axum_sqlite_review::DocumentMachine",
  "summary": {
    "breaking": 1,
    "review": 2,
    "informational": 1
  },
  "changes": [
    {
      "severity": "breaking",
      "category": "state_removed",
      "key": "Archived",
      "message": "State `Archived` was removed; persisted rows or events using this state need an explicit migration."
    },
    {
      "severity": "review",
      "category": "edge_added",
      "key": "InReview::reject->Draft",
      "message": "New legal transition edge from `InReview` through `reject` to `Draft`; review downstream authorization and side effects."
    }
  ],
  "authority": {
    "before": "cfg_pruned_macro_input",
    "after": "cfg_pruned_macro_input",
    "observation_point": "serialized_stable_graph_metadata"
  }
}
```

Severity vocabulary:

- `breaking`: likely requires migration code or release-blocking review.
- `review`: may be safe, but should be acknowledged in PR review.
- `informational`: useful context, not a workflow-legality risk by itself.

## Migration Concern Rules

State changes:

- Removed state: breaking. Persisted projections, event payloads, serialized
  state names, dashboards, and analytics filters may still refer to it.
- Added state: review. New state usually needs persistence, monitoring,
  authorization, documentation, and fixtures.
- State `has_data` changed: breaking. The v1 graph can only flag that payload
  presence changed; it cannot describe field-level migrations.
- State label or description changed: informational.

Transition-site changes:

- Removed transition site: breaking. Callers, event replay, and runtime records
  may reference the old transition id or method path.
- Added transition site: review. New entry point should be checked for
  authorization, side effects, and expected events.
- Transition label or description changed: informational.

Edge changes:

- Removed edge: breaking. Existing persisted events may no longer replay, and
  users may lose a previously legal path.
- Added edge: review. New legal path may need product, security, and data-model
  review.
- Same transition site with a changed target set: report both edge additions and
  removals, then add one grouped review note for the changed branch set.

Authority and compatibility changes:

- Different graph schema versions: breaking until a version adapter exists.
- Different authority values: review. The report must show both values and avoid
  pretending the authority surface is unchanged.
- Different unsupported-case lists: review. A new unsupported case can narrow
  what the snapshot represents; a removed unsupported case may widen what tools
  are allowed to trust.
- Unmatched package, machine key, or feature set: invalid comparison, not a
  passing diff.

## CI Usage

A repository should commit baseline snapshots for public or migration-sensitive
machines under a deterministic path such as:

```text
docs/graph-snapshots/<package>/<machine-key>/<feature-set>.json
```

A future CI job can run:

```bash
cargo statum graph --machine axum-sqlite-review --format json \
  > target/statum-graph/current/axum-sqlite-review.json
cargo statum graph diff \
  --baseline docs/graph-snapshots/statum-examples/axum-sqlite-review/default.json \
  --current target/statum-graph/current/axum-sqlite-review.json \
  --format markdown \
  --fail-on breaking
```

Recommended CI policy:

- fail on invalid snapshot comparisons
- fail on breaking graph changes unless a migration note is present
- allow review-level changes, but surface them in PR comments
- always upload the JSON diff as a CI artifact
- keep Markdown output deterministic so maintainers can compare reruns

The migration-note gate can start as a convention: require a PR to include a
file under `docs/migrations/` or a commit trailer such as
`Statum-Graph-Migration: docs/migrations/2026-06-review-workflow.md` when the
report contains breaking changes.

## PR Comment Usage

The PR bot should post or update one comment per machine. The comment should be
short enough for review, with a link to the JSON artifact for full detail.

Example Markdown:

```markdown
### Statum graph diff: `showcases::axum_sqlite_review::DocumentMachine`

Authority: serialized `StableGraphMetadata` (`cfg_pruned_macro_input` on both
sides). This is a workflow-graph diff, not a full Rust behavior diff.

| severity | change | migration concern |
| --- | --- | --- |
| breaking | removed state `Archived` | persisted rows/events may need migration |
| review | added edge `InReview::reject -> Draft` | review authorization and event handling |
| informational | label changed for `Published` | presentation-only change |

CI artifact: `statum-graph-diff/axum-sqlite-review.json`
```

Comment behavior:

- update the existing bot comment instead of creating a new comment on every run
- collapse informational-only diffs behind a `<details>` block
- keep breaking and review items visible by default
- include the authority boundary in every comment
- include a migration-note link when breaking changes are acknowledged

## Implementation Plan

1. Add snapshot wrapper structs near the stable metadata tooling, but keep the
   wrapper separate from `StableGraphMetadata` so the core graph schema remains
   reusable outside CI.
2. Add a pure diff module that accepts two parsed snapshots and returns the JSON
   report shape above.
3. Add unit tests for added/removed states, added/removed edges, data-presence
   changes, presentation-only changes, invalid machine-key comparisons, and
   authority changes.
4. Add CLI support after the pure diff is stable: `cargo statum graph diff`.
5. Add PR-comment rendering as a final layer over the JSON report rather than as
   the primary diff representation.

## Closeout Checklist For A Future Implementation

- Claimed authority surface states that the diff compares exported stable graph
  snapshots.
- Actual observation point is documented as serialized `StableGraphMetadata`, not
  raw Rust source, expanded items, or runtime values.
- Unsupported cases reject or warn instead of being omitted from the report.
- Tests include adversarial cases for duplicate transition-site keys, mismatched
  machine keys, changed authority, changed unsupported-case lists, removed states,
  and branch target-set changes.
- CI docs show both artifact generation and PR-comment usage.
