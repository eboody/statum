# statum-examples

`statum-examples` is the runnable examples crate for the Statum workspace.

The flagship workflow is document approval. Start with
`axum-sqlite-review`: it demonstrates state-specific review data, legal
`submit`/`approve` transitions, SQLite-backed rehydration, and generated graph
edges, Mermaid, and DOT output from the same machine metadata.

Use it in two modes:

- `src/toy_demos/` for small syntax-first examples
- `src/showcases/` for realistic service, CLI, worker, and protocol examples

## Run

```bash
cargo test -p statum-examples
cargo run -p statum-examples --bin axum-sqlite-review
cargo run -p statum-examples --bin mermaid-state-graph
cargo run -p statum-examples --bin dot-state-graph
cargo run -p statum-examples --bin clap-sqlite-deploy-pipeline
cargo run -p statum-examples --bin sqlite-event-log-rebuild
cargo run -p statum-examples --bin tokio-sqlite-job-runner
cargo run -p statum-examples --bin tokio-websocket-session
```

## Contents

- Toy demos:
  - `example_01_setup.rs` through `16-machine-introspection.rs`
  - best when you are learning the macros or one helper at a time
  - includes an introspection example that shows exact branch alternatives and runtime transition recording
- Showcases:
  - `axum-sqlite-review`: flagship document approval; HTTP + SQLite + typed rehydration + graph edges
  - `serde_json_snapshot`: JSON snapshot persistence; Serde deserialize -> typed rebuild -> legal checkout -> serialize
  - `clap-sqlite-deploy-pipeline`: multi-invocation CLI workflow
  - `sqlite-event-log-rebuild`: append-only projection and rebuild companion
  - `tokio-sqlite-job-runner`: retries, leases, and background work
  - `tokio-websocket-session`: protocol-safe session lifecycle
- Tests:
  - `tests/toy_demos.rs` mirrors the syntax-first examples
  - `tests/showcases.rs` exercises the realistic apps
  - `tests/patterns.rs` and `tests/stress.rs` cover broader permutations

## Repository

<https://github.com/eboody/statum>
