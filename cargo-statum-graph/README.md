# cargo-statum-graph

`cargo-statum-graph` is the zero-touch CLI for codebase-level Statum graph
export and exact-lane inspection.

It builds a temporary runner internally, links the selected crate, and writes
the combined static codebase graph as Mermaid, DOT, PlantUML, and JSON,
including declared validator-entry nodes from compiled `#[validators]` impls.
It can also launch an exact inspector TUI over that same linked compiled
`CodebaseDoc` surface.

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

That launches the exact inspector TUI for the selected workspace. The current
MVP shows:

- workspace overview with machine count and disconnected groups
- machine view with states, transitions, validator entries, and summary edges
- relation pane with inbound and outbound exact relations for the current
  machine, state, or transition
- exact-lane search plus relation-kind and relation-basis filters
- detail pane explaining the current selection, including
  `#[present(description = ...)]` text and source rustdoc (`///`) when
  available

`inspect` requires an interactive terminal on stdin and stdout.

Keybindings:

- `tab` / `shift-tab`: move focus between panes
- `h` / `l`: switch machine tabs or toggle relation direction
- `j` / `k`: move within the focused list
- `/`: enter exact-lane search mode
- `enter` / `esc`: leave search mode
- `1` / `2` / `3`: toggle payload, field, and param relation filters
- `4` / `5`: toggle direct-type and declared-reference relation-basis filters
- `0`: clear relation filters
- `q`: quit

The inspector is exact-lane only today. It consumes the linked compiled
`CodebaseDoc` surface directly; it does not do heuristic body analysis,
runtime replay, or snapshot inspection yet.

For concise labels and descriptions in the inspector, use `#[present(...)]`.
For fuller detail-pane docs that also show up in rustdoc, use outer rustdoc
comments on the machine, state variants, transition methods, and
`#[validators]` impls.
