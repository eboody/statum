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
compiled `CodebaseDoc` surface, with journey-first composition inspection as
the default home and a separate heuristic lane for broader source-scanned
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

- top-level views for `Journeys`, `Machines`, and `Topology`
- `Journeys` as the default home when the workspace has any composition
  machine
- left-pane composition machine selection plus a separate `Entry -> Exit`
  journey list for the selected machine
- grouped exact journey families for heavily branching composition machines
- center-pane exact journey projection as Mermaid `stateDiagram-v2`
  rendered through `termaid` when available
- right-pane tabs for `Steps`, `Protocols`, `Mermaid`, `Source`, and `Issues`
- a persistent journey header with machine, journey count, selected journey,
  touched protocol summary, and a fast topology jump hint
- exact journey diagrams that show only one selected finite root-to-sink
  composition trace, with numbered transition labels
- zero-step journey handling for entry-is-exit traces
- `Machines` as the full legal-state drilldown for protocol and composition
  machines
- `Topology` as secondary workspace context instead of the first screen
- topology scales for `Overview`, `Focus`, and `Full`
- `Overview` that shows the connected component for the selected machine
- `Focus` that shows the selected machine plus nearby neighbors, with a
  `1`-hop or `2`-hop radius
- entering `Topology` from `Journeys` starts in `Focus` around the selected
  composition machine
- `Full` that shows every visible machine in the linked workspace flow graph
- left-to-right and top-down topology layout toggles
- role-shaped topology nodes:
  composition machines render as double boxes and protocol machines render as
  plain boxes
- topology edges:
  owned orchestration handoffs render as thick arrows, other linked handoffs
  render as solid arrows, and static references render as dotted arrows
- machine view that renders the selected machine as an exact Mermaid
  `stateDiagram-v2`, with states, transitions, rebuild entries, handoffs, and
  journeys for drilldown
- handoff pane with inbound and outbound proven relations plus optional
  weaker source-scanned hints
- explicit empty-state guidance when the selected state or transition has no
  direct relations but the machine does
- search plus proven relation-kind filters and current relation-basis filters
  for direct-type and declared-reference relations
- heuristic evidence filters for type-surface and body matches
- `proven`, `hints`, and `both` lane toggles
- guide tabs for `Guide`, `Docs`, `Mermaid`, `Source`, and `Why`
- detail pane explaining the current selection, including
  `#[present(description = ...)]` text and source rustdoc (`///`) when
  available. For `#[via(...)]` relations, the detail pane also shows the
  attested route, producer machine, producer source state, and producer
  transition. Machine detail also shows composition diagnostics when a
  protocol machine still looks like a composition candidate. The center pane
  prefers `STATUM_TERMAID_BIN`, then an adjacent
  `../termaid/target/release/termaid`, then `termaid` on `PATH`. If no
  renderer is available, the pane falls back to raw Mermaid source and states
  why. For Mermaid flowcharts, the preview also retries a `TD` render when a
  horizontal preview fails in `termaid`, and only then falls back to raw
  Mermaid.

See [`docs/inspector-how-to-read.md`](../docs/inspector-how-to-read.md) for a
practical reading guide for `Journeys`, `Machines`, and `Topology`.

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

- `tab` / `shift-tab`: move focus between panes; in `Journeys`, that means
  machines, journeys, diagram, and detail
- `w`: cycle available workspace sections
- outline `h` / `l` or left / right: switch `Journeys` / `Machines` / `Topology`
- `[` / `]`: switch center or detail tabs
- `h` / `l`: move left and right, or pan horizontally in the center diagram
- `j` / `k`: move within the focused list, or scroll vertically in the center
  diagram
- `/`: enter search mode
- `enter` / `esc`: leave search mode
- `s`: change search scope
- `v`: cycle topology scale
- `r`: toggle topology focus radius between `1` and `2` hops
- `L`: toggle topology layout between `LR` and `TD`
- `e` / `m` / `H`: exact, mixed, and heuristic lane selection
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
