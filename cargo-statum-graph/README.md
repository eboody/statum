# cargo-statum-graph

`cargo-statum-graph` is the zero-touch CLI package for exact Statum workspace
export, exact Mermaid state and sequence diagram generation, and the Statum
Inspector TUI.

It materializes a stable generated runner under the target workspace's
`target/statum-graph/runner/<key>/`, links the selected crate inside that
workspace context, and writes the combined exact workspace graph as Mermaid,
DOT, PlantUML, and JSON, including declared validator-entry nodes from
compiled `#[validators]` impls. `inspect` and `export` reuse that cached
runner home across invocations, and `suggest` now uses that same cached
runner path too, but `CodebaseDoc::linked()` still executes fresh at runtime
on every run. It can also launch an inspector TUI over that same linked
compiled `CodebaseDoc` surface, with composition machines as the primary
workspace flow surface and a separate heuristic lane for broader source-scanned
machine coupling hints.

## Install

```text
cargo install cargo-statum-graph
```

## Export

```text
cargo statum-graph export \
  /path/to/workspace
```

That writes:

- `/path/to/workspace/codebase.mmd`
- `/path/to/workspace/codebase.dot`
- `/path/to/workspace/codebase.puml`
- `/path/to/workspace/codebase.json`

If you want a different output directory:

```text
cargo statum-graph export \
  /path/to/workspace \
  --out-dir /tmp/codebase-graph
```

If you want to narrow export to one library package inside a multi-package
workspace:

```text
cargo statum-graph export \
  /path/to/workspace \
  --package app
```

`codebase` still works as a compatibility alias in this release, but `export`
is now the primary command name.

For local development against an unreleased Statum checkout, add
`--patch-statum-root /path/to/statum`.

If the target workspace already depends on a local Statum checkout through
path dependencies, the runner detects that local Statum workspace and patches
to the same root automatically so linked inventories do not split across
different `statum-core` copies.

## State Diagram

```text
cargo statum-graph state-diagram \
  /path/to/workspace \
  --machine workflow::Machine
```

That prints one exact Mermaid `stateDiagram-v2` for the selected linked
machine.

- Selection accepts the exact linked Rust type path or one unique suffix such
  as `workflow::Machine`.
- If there is exactly one linked machine, `--machine` can be omitted.
- If more than one linked machine matches, the command fails closed and lists
  the available machine paths.

## Sequence Diagram

```text
cargo statum-graph sequence-diagram \
  /path/to/workspace \
  --relation 3
```

That prints one exact Mermaid `sequenceDiagram` for the selected exact linked
relation.

You can also select by machine pair when the pair is unique:

```text
cargo statum-graph sequence-diagram \
  /path/to/workspace \
  --from DocumentFlow \
  --to publication::Machine
```

- `--relation` is the most direct selector when you already know the exact
  relation index from export or inspection.
- `--from` and `--to` resolve by exact linked path or unique suffix.
- If a machine pair maps to more than one exact relation, the command fails
  closed and prints the matching relation summaries instead of guessing.
- Sequence export is relation-centric today. Exact composition-path sequence
  export is still future work.

## Inspect

```text
cargo statum-graph inspect \
  /path/to/workspace
```

That launches the inspector TUI for the selected workspace. The current
surface shows:

- workspace sections for `Workspace`, `Machine`, and `Gaps`
- diagram-first workspace home when any
  `#[machine(role = composition)]` machines exist
- workspace home that renders the exact linked workspace Mermaid flowchart in
  the center pane instead of leading with text cards
- machine overview that renders the selected machine as an exact Mermaid
  `stateDiagram-v2`
- path explorer that prefers composition-owned routes, then raw exact graph
  routes, then heuristic fallback when the current lane allows it
- gap view that surfaces composition warnings and heuristic-only suggestions
  together with the best currently visible path to the suggested target
- machine view with states, transitions, validator entries, relations, and
  paths for leaf protocol drilldown
- composition machines surfaced from `#[machine(role = composition)]`, with
  composition-owned direct child-machine edges labeled as `composition refs`
  instead of generic exact references
- relation pane with inbound and outbound exact relations plus optional
  heuristic machine-to-machine coupling hints
- explicit empty-state guidance when the selected state or transition has no
  direct relations but the machine does
- search plus exact relation-kind filters and current relation-basis filters
  for direct-type and declared-reference relations
- heuristic evidence filters for type-surface and body matches
- exact-only, heuristic-only, and mixed lane toggles
- detail pane explaining the current selection, including
  `#[present(description = ...)]` text and source rustdoc (`///`) when
  available. For `#[via(...)]` relations, the detail pane also shows the
  attested route, producer machine, producer source state, and producer
  transition. Composition-owned exact relations also show their composition
  semantics and source/target machine roles. Summary and exact relation cards
  now prefer those composition-owned explanations. Machine detail also shows
  composition diagnostics when a protocol machine still looks like a
  composition candidate. The detail tabs also include a `Diagram` preview for
  exact machine and exact relation selections. The inspector prefers
  `STATUM_TERMAID_BIN`, then an adjacent
  `../termaid/target/release/termaid`, then `termaid` on `PATH`. If no
  renderer is available or preview rendering fails, the pane falls back to
  raw Mermaid source and states why.

If composition machines exist, the inspector opens on `Workspace` first. If
none exist, it falls back to `Machine`.

`inspect` requires an interactive terminal on stdin and stdout.

## Suggest

```text
cargo statum-graph suggest \
  /path/to/workspace
```

That prints composition diagnostics without launching the TUI.

- `warning` means a protocol machine already exposes exact typed
  cross-machine orchestration and should likely be marked
  `#[machine(role = composition)]`.
- `suggestion` means the coupling is still only visible through the heuristic
  lane, so the next step is to model it in typed composition state or
  transition surfaces or promote a detached handoff into the exact lane.
- The report also prints heuristic collector status so a quiet suggestion list
  does not hide `partial` or `unavailable` source scanning.

Keybindings:

- `tab` / `shift-tab`: move focus between panes
- `w`: cycle available workspace sections
- `[` / `]`: switch center or detail tabs
- `j` / `k`: move within the focused list, or scroll the center diagram when
  the main view is on `Workspace` or `Diagram`
- `/`: enter search mode
- `enter` / `esc`: leave search mode
- `s`: change search scope
- `e` / `m` / `h`: exact, mixed, and heuristic lane selection
- `p` / `f` / `t`: toggle payload, field, and param relation filters
- `d` / `n`: toggle direct-type and declared-reference relation-basis filters
- `g` / `b`: toggle heuristic type-surface and body evidence filters
- `o` / `i`: outbound and inbound relation selection in `Relations`
- `0`: clear relation filters
- `?`: help
- `q`: quit

Exact lane:

- consumes the linked compiled `CodebaseDoc` surface directly
- is the only lane backed by Mermaid, DOT, PlantUML, and JSON export
- drives the composition home view and path explorer before any heuristic
  fallback is considered
- is where `#[via(...)]` relations appear with exact producer-route detail
- is where direct child-machine references on `#[machine(role = composition)]`
  machines appear as composition-owned exact relations
- fails closed on malformed exact relation inventories instead of writing a
  partial graph bundle or inspector view

Heuristic lane:

- is TUI-only
- scans raw source from the selected packages' reachable library module trees
- supports `#[path = ...] mod ...;` module edges while walking those trees
- scans state payload types, local payload structs reachable from those
  states, transition signatures and bodies, and non-transition `impl
  Machine<State>` method signatures
- resolves only to already-known exact machines
- stays machine-first in v1, so it does not claim heuristic target states or
  transitions
- hides heuristic relations in mixed mode when the exact lane already covers
  the same source machine/state-or-transition and target machine

The heuristic lane is useful but non-authoritative. It does not change
`export` output. Runtime replay and snapshot inspection are still
future work.

If you are moving a workspace from loose cross-machine coupling into typed
composition flow, start with
[docs/composition-migration.md](../docs/composition-migration.md) and the
composition example in
[statum-examples/src/toy_demos/example_18_composition_machine.rs](../statum-examples/src/toy_demos/example_18_composition_machine.rs).

If a cross-flow artifact or handoff type is stable enough to count as exact,
promote it with `#[machine_ref(...)]` on the nominal type once instead of
depending on the heuristic lane. Target the earliest stable producer state for
that artifact.

For concise labels and descriptions in the inspector, use `#[present(...)]`.
For fuller detail-pane docs that also show up in rustdoc, use outer rustdoc
comments on the machine, state variants, transition methods, and
`#[validators]` impls.
