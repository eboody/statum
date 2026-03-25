# cargo-statum-graph

`cargo-statum-graph` is the zero-touch CLI for codebase-level Statum graph
export.

It builds a temporary runner internally, links the selected crate, and writes
the combined static codebase graph as Mermaid, DOT, PlantUML, and JSON.

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
