<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./docs/static/image/logo-dark.png">
    <img alt="statum logo" src="./docs/static/image/logo.png" width="420">
  </picture>
  <p><strong>Typed workflow protocols for Rust.</strong></p>
  <p>
    <a href="https://github.com/eboody/statum/actions/workflows/ci.yml"><img src="https://github.com/eboody/statum/actions/workflows/ci.yml/badge.svg?branch=main&event=push" alt="build status" /></a>
    <a href="https://crates.io/crates/statum"><img src="https://img.shields.io/crates/v/statum.svg?logo=rust" alt="crates.io" /></a>
    <a href="https://docs.rs/statum"><img src="https://docs.rs/statum/badge.svg" alt="docs.rs" /></a>
  </p>
</div>

# Statum

Statum is a typed workflow-protocol framework for Rust. Use it when you are
modeling a concept that moves through distinct states and you want those states
encoded in the type system.

The point is the same spirit that makes `Option<T>` and `Result<T, E>` powerful:
make undesirable states unrepresentable. `Option` makes absence explicit instead
of hiding it in null. `Result` makes failure explicit instead of hiding it in an
ambient exception or sentinel value. Statum applies that idea to domain phases:
a draft document, an in-review document, and a published document can be
different Rust types with different methods and different data.

The core promise is representational correctness:

- `DocumentMachine<Draft>` can be submitted for review.
- `DocumentMachine<InReview>` can be approved.
- Reviewer assignment data only exists while the document is in review.
- Code cannot accidentally call phase-specific behavior from the wrong phase.

Statum provides this through four macros:

```text
#[state]      named protocol phases
#[machine]    shared workflow context carried across phases
#[transition] legal edges between phases
#[validators] typed rehydration from stored or projected data
```

Enable the optional `introspection` feature when you also want generated graph
metadata for docs, tests, CLIs, or review tooling.

## Install

Statum targets stable Rust and currently supports Rust `1.93+`. The repository
pins `rust-toolchain.toml` to Rust `1.96.0` for day-to-day development and keeps
`rust-version = "1.93"` in Cargo metadata for the supported minimum.

```toml
[dependencies]
statum = "0.9.0"
```

No default features are enabled. Add graph metadata when you need it:

```toml
[dependencies]
statum = { version = "0.9.0", features = ["introspection"] }
```

For the strongest graph-metadata authority boundary, enable strict mode:

```toml
[dependencies]
statum = { version = "0.9.0", features = ["strict-introspection"] }
```

`strict-introspection` only changes graph metadata generation. Unsupported
transition return shapes are rejected unless the method provides an explicit
`#[introspect(return = ...)]` annotation.

To reproduce the primary GitHub Actions gate locally:

```bash
bash scripts/check_ci_parity.sh
```

## A Small Workflow

A document approval protocol has three phases:

- `Draft`: editable content with no reviewer yet.
- `InReview`: the same document plus a required `ReviewAssignment`.
- `Published`: approved content; the reviewer field is gone again.

Statum makes those phases different Rust types and puts transitions only on the
phases that may use them:

```rust
use statum::{machine, state, transition};

#[state]
enum DocumentState {
    Draft,
    InReview(ReviewAssignment),
    Published,
}

struct ReviewAssignment {
    reviewer: String,
}

#[machine]
struct DocumentMachine<DocumentState> {
    id: i64,
    title: String,
    body: String,
}

#[transition]
impl DocumentMachine<Draft> {
    fn submit(self, reviewer: String) -> DocumentMachine<InReview> {
        self.transition_with(ReviewAssignment { reviewer })
    }
}

#[transition]
impl DocumentMachine<InReview> {
    fn approve(self) -> DocumentMachine<Published> {
        self.transition()
    }
}
```

Now the compiler enforces the workflow shape:

- `submit()` is only callable on `DocumentMachine<Draft>`.
- `approve()` is only callable on `DocumentMachine<InReview>`.
- `ReviewAssignment` is only accessible on the in-review machine.
- A persisted row can be rebuilt into `document_machine::SomeState`, matched,
  and only then transitioned by an HTTP handler or worker.

Statum is storage-agnostic. The SQLite/sqlx examples are integration patterns,
not built-in adapters.

Start with the guided document-approval walkthrough:
[docs/tutorial-review-workflow.md](docs/tutorial-review-workflow.md). The
service-shaped implementation lives in
[statum-examples/src/showcases/axum_sqlite_review.rs](statum-examples/src/showcases/axum_sqlite_review.rs).

## Typed Rehydration

Typed rehydration is the boundary feature for services that store or receive
state dynamically. The central model is still typestate: undesirable states are
unrepresentable in the core API. `#[validators]` is how a database row, event
projection, or API payload earns its way back into that typed world.

A validator block lives on the persisted type and names the machine it rebuilds:

```rust
use statum::{machine, state, validators};

#[state]
enum TaskState {
    Draft,
    InReview(ReviewData),
    Published,
}

struct ReviewData {
    reviewer: String,
}

#[machine]
struct TaskMachine<TaskState> {
    client: String,
    name: String,
}

struct DbRow {
    status: String,
}

#[validators(TaskMachine)]
impl DbRow {
    fn is_draft(&self) -> statum::Result<()> {
        (self.status == "draft")
            .then_some(())
            .ok_or(statum::Error::InvalidState)
    }

    fn is_in_review(&self) -> statum::Result<ReviewData> {
        let _ = (&client, &name); // generated machine-field bindings
        (self.status == "in_review")
            .then(|| ReviewData {
                reviewer: format!("reviewer-for-{client}"),
            })
            .ok_or(statum::Error::InvalidState)
    }

    fn is_published(&self) -> statum::Result<()> {
        (self.status == "published")
            .then_some(())
            .ok_or(statum::Error::InvalidState)
    }
}
```

Then rebuild through the machine:

```rust
let machine = TaskMachine::rebuild(&row)
    .client("acme".to_owned())
    .name("spec".to_owned())
    .build()?;

match machine {
    task_machine::SomeState::Draft(task) => { /* edit */ }
    task_machine::SomeState::InReview(task) => { /* approve */ }
    task_machine::SomeState::Published(task) => { /* serve */ }
}
```

Key options:

- Use `statum::Validation<T>` instead of `statum::Result<T>` when failed
  candidates should carry reason keys and messages into rebuild reports.
- Enable `rebuild-reports` for single-row `.build_report()` and `.explain()`.
- Enable `rebuild-batch` for `.into_machines()`, `.into_machines_by(...)`, and
  `Machine::rebuild_many(...)`.
- Project append-only event logs into validator rows first; the small
  `statum::projection` helpers cover common reductions.

Full guide: [docs/persistence-and-validators.md](docs/persistence-and-validators.md).
Event-log case study: [docs/case-study-event-log-rebuild.md](docs/case-study-event-log-rebuild.md).

## Machine Introspection

With `introspection`, Statum emits machine metadata from the active, cfg-pruned
macro input. That lets downstream tools read the workflow graph without
maintaining a parallel definition by hand.

Use it for:

- CLI explainers and generated docs.
- Graph snapshots and pull-request diffs.
- Tests that assert legal transitions.
- Replay, debugging, and review tooling.

With `strict-introspection`, supported return shapes are exact at the
transition-site level: direct `Machine<NextState>` values and canonical wrappers
around those machine types (`Option`, `Result`, and `statum::Branch`). Strict
mode is exact for macro-readable transition targets, not for runtime guards or
the semantic truth of explicit overrides.

Read [docs/introspection.md](docs/introspection.md) and
[docs/introspection-authority.md](docs/introspection-authority.md), or run
[statum-examples/src/toy_demos/16-machine-introspection.rs](statum-examples/src/toy_demos/16-machine-introspection.rs).

## When To Use Statum

Use Statum when:

- A value's phase should change what callers are allowed to do with it.
- Invalid transitions are expensive enough to prevent at compile time.
- Some data is only valid in specific states.
- Workflow order, validation order, or resolution order is stable and meaningful.
- Dynamic or persisted state needs a typed re-entry point.

Skip Statum when:

- The staging is private implementation detail inside one function.
- The legal method surface barely changes across phases.
- The workflow is highly ad hoc, user-authored, or runtime-editable.
- States are still changing faster than the API around them.

If you are comparing approaches, read
[docs/why-not-just-an-enum.md](docs/why-not-just-an-enum.md).

## Common Gotchas

**`missing fields marker and state_data`**

Your derives expanded before `#[machine]`. Put `#[machine]` above
`#[derive(...)]`:

```rust
#[machine]
#[derive(Debug, Clone)]
struct DocumentMachine<DocumentState> {
    id: i64,
    title: String,
}
```

**Transition helpers in the wrong place**

Keep non-transition helpers in normal `impl` blocks. `#[transition]` is for
protocol edges, not general utility methods.

**State shape errors**

`#[state]` accepts unit variants, single-field tuple variants, and named-field
variants. Generics on the state enum are not supported.

## Showcases

For service-shaped examples, run one of these:

```bash
cargo run -p statum-examples --bin axum-sqlite-review
cargo run -p statum-examples --bin clap-sqlite-deploy-pipeline
cargo run -p statum-examples --bin sqlite-event-log-rebuild
cargo run -p statum-examples --bin tokio-sqlite-job-runner
cargo run -p statum-examples --bin tokio-websocket-session
```

- `axum-sqlite-review`: row rehydration before each HTTP transition.
- `clap-sqlite-deploy-pipeline`: repeated CLI invocations and rollback phases.
- `sqlite-event-log-rebuild`: append-only event storage and batch rebuilds.
- `tokio-sqlite-job-runner`: retries, leases, async effects, and worker loops.
- `tokio-websocket-session`: protocol-safe frames and session lifecycle phases.

## Learn More

Start with docs by job, not by macro name:

- Documentation map: [docs/README.md](docs/README.md)
- First workflow: [docs/start-here.md](docs/start-here.md),
  [docs/tutorial-review-workflow.md](docs/tutorial-review-workflow.md), and
  [docs/why-not-just-an-enum.md](docs/why-not-just-an-enum.md)
- Builders and phase data: [docs/generated-builder-reference.md](docs/generated-builder-reference.md),
  [docs/typestate-builder-design-playbook.md](docs/typestate-builder-design-playbook.md), and
  [docs/builder-ux-positioning.md](docs/builder-ux-positioning.md)
- Rehydration: [docs/persistence-and-validators.md](docs/persistence-and-validators.md),
  [docs/rehydration-vocabulary.md](docs/rehydration-vocabulary.md), and
  [docs/migration.md](docs/migration.md)
- Graph metadata: [docs/introspection.md](docs/introspection.md),
  [docs/introspection-authority.md](docs/introspection-authority.md), and
  [docs/mcp-protocol-resource-design.md](docs/mcp-protocol-resource-design.md)
- Agent adoption kit: [docs/agents/README.md](docs/agents/README.md)
- Escape-hatch audit: [docs/escape-hatches.md](docs/escape-hatches.md)

Examples and API references:

- Toy demos: [statum-examples/src/toy_demos/](statum-examples/src/toy_demos/)
- Showcase apps: [statum-examples/src/showcases/](statum-examples/src/showcases/)
- Crate docs: [statum](https://docs.rs/statum),
  [statum-core](https://docs.rs/statum-core),
  [statum-macros](https://docs.rs/statum-macros)

## Stability

- Development toolchain: Rust `1.96.0` via `rust-toolchain.toml`.
- MSRV: Rust `1.93`, declared as workspace `rust-version = "1.93"` and checked
  in CI with Rust `1.93.1`.
- Edition split: `statum` and `statum-core` use Rust 2021;
  `statum-macros`, `statum-examples`, and `cargo-statum` use Rust 2024.
