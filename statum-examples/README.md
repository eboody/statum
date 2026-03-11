# statum-examples

`statum-examples` is a runnable examples crate for the Statum workspace.

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

- Small syntax-first demos in `src/toy_demos/`
- Service-shaped showcase apps in `src/showcases/`
- A multi-invocation CLI showcase in `src/bin/clap-sqlite-deploy-pipeline.rs`
- An append-only event-log showcase in `src/bin/sqlite-event-log-rebuild.rs`
- A session-protocol showcase in `src/bin/tokio-websocket-session.rs`
- Integration-style scenario tests in `tests/toy_demos.rs`, `tests/showcases.rs`, `tests/patterns.rs`, and `tests/stress.rs`

## Repository

<https://github.com/eboody/statum>
