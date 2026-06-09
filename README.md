<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./docs/static/image/logo-dark.png">
    <img alt="statum logo" src="./docs/static/image/logo.png" width="420">
  </picture>
  <p>Statum is a typed workflow-protocol framework for Rust: phases, legal transitions, phase-specific data, typed rehydration, and optional graph introspection.</p>
  <p>
    <a href="https://github.com/eboody/statum/actions/workflows/ci.yml"><img src="https://github.com/eboody/statum/actions/workflows/ci.yml/badge.svg?branch=main&event=push" alt="build status" /></a>
    <a href="https://crates.io/crates/statum"><img src="https://img.shields.io/crates/v/statum.svg?logo=rust" alt="crates.io" /></a>
    <a href="https://docs.rs/statum"><img src="https://docs.rs/statum/badge.svg" alt="docs.rs" /></a>
  </p>
</div>

# Statum

Statum is a typed workflow-protocol framework for Rust. It is for systems where
an entity moves through named phases, only some transitions are legal, and each
phase may carry data that should not exist anywhere else.

The category is narrower than a general builder or validation library. Statum
models long-lived protocols: document approval, deployment pipelines, job
leases, sessions, and other flows where persisted facts must be rebuilt into a
safe typed state before code is allowed to continue.

The core promise is representational correctness. A `DocumentMachine<Draft>` can
be submitted for review, a `DocumentMachine<InReview>` can be approved, and
reviewer assignment data only exists in the in-review phase. Rows, events, and
other dynamic inputs stay raw until your `#[validators]` accept them as one
legal machine state.

Today's API packages that with `#[state]`, `#[machine]`, `#[transition]`, and
`#[validators]`: phases, shared machine context, legal edges, and typed
rehydration. With the `introspection` feature enabled, the same workflow
definition also emits generated graph metadata.

## Install

Statum targets stable Rust and currently supports Rust `1.93+`. The repo pins
`rust-toolchain.toml` to Rust `1.96.0` for day-to-day development and keeps
`rust-version = "1.93"` in Cargo metadata for the supported minimum.

```toml
[dependencies]
statum = "0.9.0"
```

No default features are enabled, so the basic install provides typestate,
transitions, and typed rehydration without graph metadata. Enable
`introspection` when you want generated machine graphs:

```toml
[dependencies]
statum = { version = "0.9.0", features = ["introspection"] }
```

For the strongest introspection guarantee, enable strict mode:

```toml
[dependencies]
statum = { version = "0.9.0", features = ["strict-introspection"] }
```

Compared with `introspection`, `strict-introspection` only changes the
authority boundary for generated graph metadata: unsupported return shapes are
rejected unless the transition provides an explicit
`#[introspect(return = ...)]` annotation.

To reproduce the main GitHub Actions gate locally, run:

```bash
bash scripts/check_ci_parity.sh
```

That script runs the README/doc checks, diagnostics coverage, clippy, macro UI
tests, workspace tests, hygiene checks, and docs build used by the primary CI
job.

## 60-Second Lifecycle

A document approval protocol has three phases:

- `Draft`: editable content with no reviewer yet.
- `InReview`: the same document plus a required `ReviewAssignment`.
- `Published`: approved content; the reviewer field is cleared again.

Statum makes those phases different Rust types and puts the legal edges on the
only phases that can use them:

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

Now the type system carries the protocol rules:

- `submit()` is only callable on `DocumentMachine<Draft>`.
- `approve()` is only callable on `DocumentMachine<InReview>`.
- `ReviewAssignment` only exists while the document is in review.
- A persisted row can be rebuilt into `document_machine::SomeState`, then
  matched before an HTTP handler calls the next legal transition. Statum is
  storage-agnostic; the SQLite/sqlx examples show one integration pattern, not a
  built-in adapter.
- With the `introspection` or `strict-introspection` feature enabled,
  `MachineIntrospection::GRAPH` exposes the generated `Draft --submit-->
  InReview --approve--> Published` graph for docs, tests, and tooling.

Example: [statum-examples/src/showcases/axum_sqlite_review.rs](statum-examples/src/showcases/axum_sqlite_review.rs)

## What The Compiler Enforces

The lifecycle example above is small. The point is not the syntax. The point is
that legal and illegal workflow states stop looking the same in your API.

The phase shape becomes part of the type system instead of hiding in status
enums, optional fields, and comments:

- `DocumentMachine<Draft>` and `DocumentMachine<InReview>` are different types.
- `submit()` only exists on `DocumentMachine<Draft>`.
- `approve()` only exists on `DocumentMachine<InReview>`.
- If a phase carries data, that data only exists when the machine is actually
  in that phase.

This is the point of Statum: only legal, understood workflow states become
first-class values. Raw rows and event projections stay raw until
your `#[validators]` accept them as typed machines.

If you add derives, place them below `#[state]` and `#[machine]`:

```rust
# use statum::{machine, state};
# #[state]
# enum DocumentState {
#     Draft,
# }
#[machine]
#[derive(Debug, Clone)]
struct DocumentMachine<DocumentState> {
    id: i64,
    title: String,
}

# fn main() {}
```

That avoids the common `missing fields marker and state_data` error.

## Mental Model

Use Statum when pressing `.` before and after a workflow phase change should
show a meaningfully different method surface.

The current macro surface is machine-shaped:

```text
#[state]      -> named protocol phases
#[machine]    -> shared workflow context carried across phases
#[transition] -> legal edges between phases
#[validators] -> typed rehydration from stored data
```

Roughly, Statum generates:

- Marker types for each state variant, such as `Draft`, `InReview`, and
  `Published`.
- A machine type parameterized by the current state, with hidden `marker` and
  `state_data` fields.
- Builders for constructing new workflow machines, such as
  `DocumentMachine::<Draft>::builder()`.
- A machine-scoped enum like `document_machine::SomeState` for matching
  reconstructed machines.
  `document_machine::State` remains an alias for compatibility.
- With `rebuild-batch`, a machine-scoped `document_machine::Fields` struct for
  batch rebuilds where each row needs different machine context.
- With `rebuild-batch`, a machine-scoped batch rehydration trait like
  `document_machine::IntoMachinesExt`.

This is the core model. The rest of the crate is about making those four pieces
ergonomic.

> Typed rehydration is the unusual part: if you already have rows, events, or
> persisted workflow data, `#[validators]` can rebuild them into typed machines.
> Full example below.

If you are evaluating Statum from the outside, start with
[docs/start-here.md](docs/start-here.md). The canonical workflow is document
approval: a draft gains an assigned reviewer, can only be approved while it is
in review, is rebuilt from persisted rows before HTTP handlers transition it,
and exports its generated machine graph for tooling. Read the guided walkthrough
in [docs/tutorial-review-workflow.md](docs/tutorial-review-workflow.md), then run
the service-shaped example in
[statum-examples/src/showcases/axum_sqlite_review.rs](statum-examples/src/showcases/axum_sqlite_review.rs).

If your first question is whether Statum is the right tool, read
[docs/why-not-just-an-enum.md](docs/why-not-just-an-enum.md). It compares plain
enums, runtime state machines, builder crates, runtime validation, and Statum
typestate with broken and valid workflow examples.

## Machine Introspection

Statum can also emit typed machine introspection directly from the active
cfg-pruned macro input for supported syntax. Use it when downstream tooling
needs the machine structure without rebuilding a parallel graph table by hand:
CLI explainers, generated docs, graph exports, branch-strip views, test
assertions about strict-mode legal transitions, and replay or debug tooling.

With `strict-introspection` enabled, the generated graph is exact at the
transition-site level. In that mode, introspection semantics come only from
directly readable `#[transition]` signatures or explicit
`#[introspect(return = ...)]` escape hatches. Supported return shapes are
direct `Machine<NextState>` values plus canonical wrapper paths around those
machine types: `::core::option::Option<...>`, `::core::result::Result<..., E>`,
and `::statum::Branch<..., ...>`. Unsupported custom decision enums, wrapper
aliases, and differently-qualified machine paths are rejected instead of
approximated.

Without `strict-introspection`, Statum still follows some source-backed aliases
for ergonomics. That default mode is useful metadata, but the exact-authority
claim belongs to strict mode. Strict mode is exact for macro-readable transition
targets, not for runtime guards or the semantic truth of explicit overrides.

Whole-item `#[cfg]` gates are supported, but nested `#[cfg]` or `#[cfg_attr]`
on `#[state]` variants, variant payload fields, or `#[machine]` fields are
rejected because they would otherwise drift the generated metadata from the
active build.

See [docs/introspection.md](docs/introspection.md) for the full guide,
[docs/introspection-authority.md](docs/introspection-authority.md) for the
metadata authority boundary, and
[statum-examples/src/toy_demos/16-machine-introspection.rs](statum-examples/src/toy_demos/16-machine-introspection.rs)
for a runnable example.

For source-local labels and descriptions, use `#[present(...)]` on the machine,
state variants, and transition methods. If you also want typed metadata in the
generated `machine::PRESENTATION` constant, declare
`#[presentation_types(machine = ..., state = ..., transition = ...)]` on the
machine and add `metadata = ...` to each annotated item in the typed
categories. Manual `MachinePresentation` overlays still remain first-class when
the generated sugar is not the right fit.

## Typed Rehydration

`#[validators]` is the feature that turns stored data back into typed machines. Each `is_*` method checks whether the persisted value belongs to a state, returns `()` or state-specific data, and Statum builds the right typed output. Use `#[validators(Machine)]` for same-module machines and anchored paths like `#[validators(self::flow::Machine)]`, `#[validators(super::flow::Machine)]`, or `#[validators(crate::flow::Machine)]` when the machine lives elsewhere. In relaxed mode, bare multi-segment paths like `#[validators(flow::Machine)]` are treated as local child-module paths, not imported aliases or re-exports. If Statum cannot resolve that local path, it fails with a diagnostic that asks for an anchored path instead:

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

enum Status {
    Draft,
    InReview,
    Published,
}

struct DbRow {
    status: Status,
}

#[validators(TaskMachine)]
impl DbRow {
    fn is_draft(&self) -> statum::Result<()> {
        let _ = (&client, &name);
        if matches!(self.status, Status::Draft) {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_in_review(&self) -> statum::Result<ReviewData> {
        let _ = &name;
        if matches!(self.status, Status::InReview) {
            Ok(ReviewData {
                reviewer: format!("reviewer-for-{client}"),
            })
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_published(&self) -> statum::Result<()> {
        if matches!(self.status, Status::Published) {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }
}

fn main() -> statum::Result<()> {
    let row = DbRow {
        status: Status::InReview,
    };

    let machine = TaskMachine::rebuild(&row)
        .client("acme".to_owned())
        .name("spec".to_owned())
        .build()?;

    match machine {
        task_machine::SomeState::Draft(_) => {}
        task_machine::SomeState::InReview(task) => {
            assert_eq!(task.state_data.reviewer.as_str(), "reviewer-for-acme");
        }
        task_machine::SomeState::Published(_) => {}
    }

    Ok(())
}
```

`TaskMachine::rebuild(...)` is the core type-first entry point for one row.
`row.into_machine()` remains supported as a fallback. Collection rebuild helpers
(`.into_machines()`, `.into_machines_by(...)`, and
`TaskMachine::rebuild_many(...)`) are available with the `rebuild-batch` Cargo
feature.

Key details:

- Validator methods run against your persisted type and return either `statum::Result<T>` for simple yes/no membership or `statum::Validation<T>` when a failed match should carry a stable reason key and optional message into rebuild reports.
- Machine fields are available by name inside validator methods through generated bindings, so `client` and `name` are usable without boilerplate parameter plumbing. Persisted-row fields still live on `self`.
- Unit states return `statum::Result<()>` or `statum::Validation<()>`; data-bearing states return `statum::Result<StateData>` or `statum::Validation<StateData>`.
- With the `rebuild-reports` Cargo feature, `.build_report()` keeps the same rebuild semantics as `.build()`, but also records validator attempts in order. Collection reports through `.build_reports()` additionally require `rebuild-batch`. Diagnostic validators populate `RebuildAttempt.reason_key` and `RebuildAttempt.message`.
- With `rebuild-reports`, `.explain()` is the single-row candidate explain mode: it evaluates every candidate state, returns a non-throwing `RebuildReport`, and marks ambiguity when more than one validator accepts the row.
- `.build()` returns the generated wrapper enum, which you can match as `task_machine::SomeState`.
  `task_machine::State` is kept as an alias so older code still compiles.
- If any validator is `async`, the generated builder becomes `async`.
- With `rebuild-batch`, use `.into_machines_by(|row| task_machine::Fields { ... })` when batch reconstruction needs different machine fields per row.
- For append-only event logs, project events into validator rows first. `statum::projection::reduce_one` and `reduce_grouped` are the small helper layer for that.
- If no validator matches, `.build()` and `build_report().into_result()` both return `statum::Error::InvalidState`.

Examples: the flagship document-approval path is [docs/tutorial-review-workflow.md](docs/tutorial-review-workflow.md) plus [statum-examples/src/showcases/axum_sqlite_review.rs](statum-examples/src/showcases/axum_sqlite_review.rs). Focused API demos remain in [statum-examples/src/toy_demos/09-persistent-data.rs](statum-examples/src/toy_demos/09-persistent-data.rs), [statum-examples/src/toy_demos/10-persistent-data-vecs.rs](statum-examples/src/toy_demos/10-persistent-data-vecs.rs), and [statum-examples/src/toy_demos/14-batch-machine-fields.rs](statum-examples/src/toy_demos/14-batch-machine-fields.rs). The event-log companion is [statum-examples/src/showcases/sqlite_event_log_rebuild.rs](statum-examples/src/showcases/sqlite_event_log_rebuild.rs).

More detail: [docs/persistence-and-validators.md](docs/persistence-and-validators.md).
If you are auditing proof-bypass or metadata-override surfaces, see the
grep-able [escape hatch catalog](docs/escape-hatches.md).

## Core Rules

`#[state]`

- Apply it to an enum.
- Variants must be unit variants, single-field tuple variants, or named-field variants.
- Generics on the state enum are not supported.

`#[machine]`

- Apply it to a struct.
- The first generic parameter must match the `#[state]` enum name.
- Additional type and const generics are supported after the state generic.
- Extra machine lifetime generics are effectively unavailable because Rust
  requires lifetimes before type parameters, and Statum reserves the first
  generic slot for the state family.
- Put `#[machine]` above `#[derive(...)]`.

`#[transition]`

- Apply it to `impl Machine<State>` blocks that define legal transitions.
- Transition methods must take `self` or `mut self`.
- Return `Machine<NextState>` directly, or wrap it in canonical `::core::result::Result`, `::core::option::Option`, or `::statum::Branch` when the transition is conditional.
- Use `transition_with(data)` when the target state carries data.

`#[validators]`

- Use `#[validators(Machine)]` on an `impl` block for your persisted type.
- Define one `is_{state}` method per state variant.
- Return `statum::Result<()>` or `statum::Validation<()>` for unit states.
- Return `statum::Result<StateData>` or `statum::Validation<StateData>` for
  data-bearing states.
- Prefer `Machine::rebuild(&row)` for single-item reconstruction.
- `row.into_machine()` remains supported as the fallback entrypoint.
- Enable `rebuild-batch` for collection helpers.
- With `rebuild-batch`, call `.into_machines()` for collections that share machine fields.
- With `rebuild-batch`, `Machine::rebuild_many(rows)` is the matching type-first batch entrypoint.
- With `rebuild-batch`, call `.into_machines_by(|row| machine::Fields { ... })` when machine fields vary per item.
- Enable `rebuild-reports` for single-row `build_report()`.
- Enable both `rebuild-batch` and `rebuild-reports` for collection `build_reports()`.
- From other modules, import `machine::IntoMachinesExt as _` first.

## When To Use Statum

Use Statum when:

- You care about representational correctness and want invalid, undesirable, or
  not-yet-validated states out of the core API.
- A value's phase should change what callers are allowed to do with it.
- Workflow order, validation order, or resolution order is stable and meaningful.
- Invalid transitions are expensive.
- Available methods should change by phase.
- Some data is only valid in specific states.

Skip Statum when:

- The staging is private implementation detail inside one function or module.
- The legal method surface barely changes across phases.
- The workflow is highly ad hoc or user-authored.
- The workflow is dominated by large runtime branching or dynamic graph edits.
- States are still changing faster than the API around them.

More design guidance: [docs/typestate-builder-design-playbook.md](docs/typestate-builder-design-playbook.md)
and [docs/generated-builder-reference.md](docs/generated-builder-reference.md)

## Common Gotchas

**`missing fields marker and state_data`**

Your derives expanded before `#[machine]`. Put `#[machine]` above `#[derive(...)]`.

**Transition helpers in the wrong place**

Keep non-transition helpers in normal `impl` blocks. `#[transition]` is for protocol edges, not general utility methods.

**State shape errors**

`#[state]` accepts unit variants, single-field tuple variants, and named-field variants.

## Showcases

For real service-shaped examples, run one of these:

```bash
cargo run -p statum-examples --bin axum-sqlite-review
cargo run -p statum-examples --bin clap-sqlite-deploy-pipeline
cargo run -p statum-examples --bin sqlite-event-log-rebuild
cargo run -p statum-examples --bin tokio-sqlite-job-runner
cargo run -p statum-examples --bin tokio-websocket-session
```

- `axum-sqlite-review` demonstrates `#[validators]` rebuilding typed machines from database rows before each HTTP transition.
- `clap-sqlite-deploy-pipeline` demonstrates repeated CLI invocations, SQLite-backed typed rehydration, and explicit apply/failure/rollback phases.
- `sqlite-event-log-rebuild` demonstrates append-only event storage, projection-based typed rehydration, and batch `.into_machines()` reconstruction.
- `tokio-sqlite-job-runner` demonstrates retries, leases, async side effects, and typed rehydration in a background worker loop.
- `tokio-websocket-session` demonstrates protocol-safe frame handling, phase-gated behavior, and a session lifecycle that is not persistence-driven.

Start with document approval. It is the canonical example used across the
README, docs, and examples because it shows state-specific review data, legal
HTTP transitions, row rehydration, and graph output in one small workflow:
[docs/tutorial-review-workflow.md](docs/tutorial-review-workflow.md).

Use `sqlite-event-log-rebuild` as the persistence companion when you specifically
want append-only projection and batch rebuilds:
[docs/case-study-event-log-rebuild.md](docs/case-study-event-log-rebuild.md).

## Use With Coding Agents

If you use coding agents, Statum ships an adoption kit with copyable instruction
templates, audit heuristics, and prompts for targeted refactors and reviews.
Start with [docs/agents/README.md](docs/agents/README.md).

If you are starting from an architecture memo or protocol guide rather than
from code, use the prompts under `docs/agents/prompts/`. If you use Codex
locally, an explicit `statum-skill` works well as a deeper layer on top
of the conservative templates in this repo.

## Learn More

Start with the docs by job, not by macro name:

- Documentation map: [docs/README.md](docs/README.md)
- Start a workflow protocol: [docs/start-here.md](docs/start-here.md),
  [docs/tutorial-review-workflow.md](docs/tutorial-review-workflow.md), and
  [docs/why-not-just-an-enum.md](docs/why-not-just-an-enum.md)
- Carry phase data safely: [docs/generated-builder-reference.md](docs/generated-builder-reference.md),
  [docs/typestate-builder-design-playbook.md](docs/typestate-builder-design-playbook.md),
  and [docs/builder-ux-positioning.md](docs/builder-ux-positioning.md)
- Rehydrate persisted state: [docs/persistence-and-validators.md](docs/persistence-and-validators.md),
  [docs/rehydration-vocabulary.md](docs/rehydration-vocabulary.md),
  [docs/escape-hatches.md](docs/escape-hatches.md), and
  [docs/migration.md](docs/migration.md)
- Process batches and event logs: [docs/case-study-event-log-rebuild.md](docs/case-study-event-log-rebuild.md)
  and [docs/batch-rehydration-design.md](docs/batch-rehydration-design.md)
- Explain or generate metadata: [docs/introspection.md](docs/introspection.md),
  [docs/introspection-authority.md](docs/introspection-authority.md),
  [docs/mcp-protocol-resource-design.md](docs/mcp-protocol-resource-design.md),
  and [docs/agents/README.md](docs/agents/README.md)
- Diagnose, migrate, or measure Statum itself:
  [docs/diagnostics-quality-audit.md](docs/diagnostics-quality-audit.md),
  [docs/compile-time-benchmark-reporting.md](docs/compile-time-benchmark-reporting.md),
  [docs/compile-time-benchmark-baseline.md](docs/compile-time-benchmark-baseline.md), and
  [docs/world-class-roadmap.md](docs/world-class-roadmap.md)

Examples and API references:

- Toy demos: [statum-examples/src/toy_demos/](statum-examples/src/toy_demos/)
- Showcase apps: [statum-examples/src/showcases/](statum-examples/src/showcases/)
- Review showcase binary: [statum-examples/src/bin/axum-sqlite-review.rs](statum-examples/src/bin/axum-sqlite-review.rs)
- Deploy pipeline binary: [statum-examples/src/bin/clap-sqlite-deploy-pipeline.rs](statum-examples/src/bin/clap-sqlite-deploy-pipeline.rs)
- Event log binary: [statum-examples/src/bin/sqlite-event-log-rebuild.rs](statum-examples/src/bin/sqlite-event-log-rebuild.rs)
- Job runner binary: [statum-examples/src/bin/tokio-sqlite-job-runner.rs](statum-examples/src/bin/tokio-sqlite-job-runner.rs)
- Session binary: [statum-examples/src/bin/tokio-websocket-session.rs](statum-examples/src/bin/tokio-websocket-session.rs)
- Crate docs: [statum](https://docs.rs/statum), [statum-core](https://docs.rs/statum-core), [statum-macros](https://docs.rs/statum-macros)
- API docs: [docs.rs/statum](https://docs.rs/statum)

## Stability

- Stable Rust `1.96.0` is the day-to-day target via `rust-toolchain.toml`.
- MSRV: Rust `1.93`, declared as workspace `rust-version = "1.93"` and checked
  in CI with Rust `1.93.1`.
- Edition split: `statum` and `statum-core` use Rust 2021;
  `statum-macros`, `statum-examples`, and `cargo-statum` use Rust 2024.
