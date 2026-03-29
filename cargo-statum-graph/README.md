# cargo-statum-graph

`cargo-statum-graph` is the zero-touch CLI for codebase-level Statum graph
export and inspector TUI workflows.

It builds a temporary runner internally, links the selected crate, and writes
the combined static codebase graph as Mermaid, DOT, PlantUML, and JSON,
including declared validator-entry nodes from compiled `#[validators]` impls.
It can also launch an inspector TUI over that same linked compiled
`CodebaseDoc` surface, with declared workspace journeys above the exact graph
and a separate heuristic lane for broader source-scanned machine coupling
hints.

## Install

```text
cargo install cargo-statum-graph
```

## Usage

```text
cargo statum-graph codebase \
  /path/to/workspace
```

That writes:

- `/path/to/workspace/codebase.mmd`
- `/path/to/workspace/codebase.dot`
- `/path/to/workspace/codebase.puml`
- `/path/to/workspace/codebase.json`

If you want a different output directory:

```text
cargo statum-graph codebase \
  /path/to/workspace \
  --out-dir /tmp/codebase-graph
```

If you want to narrow export to one library package inside a multi-package
workspace:

```text
cargo statum-graph codebase \
  /path/to/workspace \
  --package app
```

For local development against an unreleased Statum checkout, add
`--patch-statum-root /path/to/statum`.

## Inspect

```text
cargo statum-graph inspect \
  /path/to/workspace
```

That launches the inspector TUI for the selected workspace. The current
surface shows:

- workspace overview with machine count and disconnected groups
- optional workspace `Journeys` section driven by declared `statum::journeys!`
  narratives
- machine view with states, transitions, validator entries, and summary edges
  that default to `Summary` when a machine has visible relationships
- composition machines surfaced from `#[machine(role = composition)]`, with
  composition-owned direct child-machine edges labeled as `composition refs`
  instead of generic exact references
- journey view with ordered entry-to-outcome cards and exact, declared,
  heuristic, or missing segment coverage
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
  semantics and source/target machine roles. Machine detail also shows
  composition diagnostics when a protocol machine still looks like a
  composition candidate. Journey detail also shows bridge types, machine-ref
  targets, and exact, declared, heuristic, or missing segment coverage.

If declared journeys exist, the inspector opens on `Journeys` first. If none
exist, it keeps the current machine-first behavior.

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
- `w`: toggle the workspace list between `Machines` and `Journeys`
- `h` / `l`: switch machine tabs or toggle relation direction
- `j` / `k`: move within the focused list
- `/`: enter search mode
- `enter` / `esc`: leave search mode
- `m`: cycle exact, heuristic, and mixed lanes
- `1` / `2` / `3`: toggle payload, field, and param relation filters
- `4` / `5`: toggle direct-type and declared-reference relation-basis filters
- `6` / `7`: toggle heuristic type-surface and body evidence filters
- `0`: clear relation filters
- `q`: quit

Exact lane:

- consumes the linked compiled `CodebaseDoc` surface directly
- is the only lane backed by Mermaid, DOT, PlantUML, and JSON export
- is where `#[via(...)]` relations appear with exact producer-route detail
- is where direct child-machine references on `#[machine(role = composition)]`
  machines appear as composition-owned exact relations
- fails closed on malformed exact relation inventories instead of writing a
  partial graph bundle or inspector view

Declared journeys:

- are inspector-only in v1
- are registered through `statum::journeys!`
- sit above the exact graph instead of changing it
- can reference machines, states, validator entry surfaces, and declared
  bridge types
- classify each segment as exact, declared bridge, heuristic cover, or missing
- do not change Mermaid, DOT, PlantUML, JSON, or `CodebaseDoc`

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
`codebase` export output. Runtime replay and snapshot inspection are still
future work.

If a cross-flow artifact or handoff type is stable enough to count as exact,
promote it with `#[machine_ref(...)]` on the nominal type once instead of
depending on the heuristic lane. Target the earliest stable producer state for
that artifact.

For concise labels and descriptions in the inspector, use `#[present(...)]`.
For fuller detail-pane docs that also show up in rustdoc, use outer rustdoc
comments on the machine, state variants, transition methods, and
`#[validators]` impls.
