# cargo-statum-graph

`cargo-statum-graph` is the zero-touch CLI for codebase-level Statum graph
export and inspector TUI workflows.

It builds a temporary runner internally, links the selected crate, and writes
the combined static codebase graph as Mermaid, DOT, PlantUML, and JSON,
including declared validator-entry nodes from compiled `#[validators]` impls.
It can also launch an inspector TUI over that same linked compiled
`CodebaseDoc` surface, with a separate heuristic lane for broader
source-scanned machine coupling hints.

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
- machine view with states, transitions, validator entries, and summary edges
- relation pane with inbound and outbound exact relations plus optional
  heuristic machine-to-machine coupling hints
- search plus exact relation-kind filters and current relation-basis filters
  for direct-type and declared-reference relations
- heuristic evidence filters for type-surface and body matches
- exact-only, heuristic-only, and mixed lane toggles
- detail pane explaining the current selection, including
  `#[present(description = ...)]` text and source rustdoc (`///`) when
  available. For `#[via(...)]` relations, the detail pane also shows the
  attested route, producer machine, producer source state, and producer
  transition.

`inspect` requires an interactive terminal on stdin and stdout.

Keybindings:

- `tab` / `shift-tab`: move focus between panes
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
