# statum-examples

`statum-examples` is the runnable examples crate for the Statum workspace.

Use it in two modes:

- `src/toy_demos/` for small syntax-first examples
- `src/showcases/` for realistic service, CLI, worker, and protocol examples

## Run

```bash
cargo test -p statum-examples
cargo run -p statum-examples --bin axum-sqlite-review
cargo run -p statum-examples --bin clap-sqlite-deploy-pipeline
cargo run -p statum-examples --bin sqlite-event-log-rebuild
cargo run -p statum-examples --bin tokio-sqlite-job-runner
cargo run -p statum-examples --bin tokio-websocket-session
```

## Contents

- Toy demos:
  - `example_01_setup.rs` through `example_18_composition_machine.rs`
  - best when you are learning the macros or one helper at a time
  - includes an introspection example that shows exact branch alternatives and runtime transition recording
  - includes an attested-composition example that shows `*_and_attest()`, `#[via(...)]`, generated `.from_*()` binders, and the resulting exact linked relation metadata
  - includes a composition-machine example that shows how a top-level workflow can be modeled as `#[machine(role = composition)]` with direct child-machine orchestration plus one detached handoff
- Showcases:
  - `axum-sqlite-review`: HTTP + SQLite + typed rehydration
  - `clap-sqlite-deploy-pipeline`: multi-invocation CLI workflow
  - `sqlite-event-log-rebuild`: append-only projection and rebuild
  - `tokio-sqlite-job-runner`: retries, leases, and background work
  - `tokio-websocket-session`: protocol-safe session lifecycle
- Tests:
  - `tests/toy_demos.rs` mirrors the syntax-first examples
  - `tests/showcases.rs` exercises the realistic apps
  - `tests/patterns.rs` and `tests/stress.rs` cover broader permutations

## Repository

<https://github.com/eboody/statum>

See also:

- [Composition Machine Migration](../docs/composition-migration.md)
- [Composition machine toy demo](src/toy_demos/example_18_composition_machine.rs)
