<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./docs/static/image/logo-dark.png">
    <img alt="statum logo" src="./docs/static/image/logo.png" width="420">
  </picture>
  <p>Statum is a framework for building protocol-safe, compile-time verified typestate workflows in Rust.</p>
  <p>
    <a href="https://github.com/eboody/statum/actions/workflows/ci.yml"><img src="https://github.com/eboody/statum/actions/workflows/ci.yml/badge.svg?branch=main&event=push" alt="build status" /></a>
    <a href="https://crates.io/crates/statum"><img src="https://img.shields.io/crates/v/statum.svg?logo=rust" alt="crates.io" /></a>
    <a href="https://docs.rs/statum"><img src="https://docs.rs/statum/badge.svg" alt="docs.rs" /></a>
  </p>
</div>

# Statum

Statum helps you model workflows where phase order matters and invalid transitions are expensive. You describe lifecycle phases with `#[state]`, durable context with `#[machine]`, legal moves with `#[transition]`, and typed rehydration from existing data with `#[validators]`.

It is opinionated on purpose: explicit transitions, state-specific data, and compile-time method gating. If that is the shape of your problem, the API stays small and the safety payoff is high.

## 60-Second Example

```rust
use statum::{machine, state, transition};

#[state]
enum LightState {
    Off,
    On,
}

#[machine]
struct LightSwitch<LightState> {
    name: String,
}

#[transition]
impl LightSwitch<Off> {
    fn switch_on(self) -> LightSwitch<On> {
        self.transition()
    }
}

#[transition]
impl LightSwitch<On> {
    fn switch_off(self) -> LightSwitch<Off> {
        self.transition()
    }
}

fn main() {
    let light = LightSwitch::<Off>::builder()
        .name("desk lamp".to_owned())
        .build();

    let light = light.switch_on();
    let _light = light.switch_off();
}
```

Example: [statum-examples/src/examples/example_01_setup.rs](statum-examples/src/examples/example_01_setup.rs)

If you add derives, place them below `#[state]` and `#[machine]`:

```rust
#[machine]
#[derive(Debug, Clone)]
struct LightSwitch<LightState> {
    name: String,
}
```

That avoids the common `missing fields marker and state_data` error.

## Mental Model

```text
#[state]      -> lifecycle phases
#[machine]    -> durable machine context
#[transition] -> legal edges between phases
#[validators] -> typed rehydration from stored data
```

Roughly, Statum generates:

- Marker types for each state variant, such as `Off` and `On`.
- A machine type parameterized by the current state, with hidden `marker` and `state_data` fields.
- Builders for new machines, such as `LightSwitch::<Off>::builder()`.
- A machine-scoped enum like `task_machine::State` for matching reconstructed machines.
- A machine-scoped batch rehydration trait like `task_machine::IntoMachinesExt`.

This is the whole model. The rest of the crate is about making those four pieces ergonomic.

> Typed rehydration is the unusual part: if you already have rows, events, or persisted workflow data, `#[validators]` can rebuild them into typed machines. Full example below.

## Typed Rehydration

`#[validators]` is the feature that turns stored data back into typed machines. Each `is_*` method checks whether the persisted value belongs to a state, returns `()` or state-specific data, and Statum builds the right typed output:

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

    let machine = row
        .into_machine()
        .client("acme".to_owned())
        .name("spec".to_owned())
        .build()?;

    match machine {
        task_machine::State::Draft(_) => {}
        task_machine::State::InReview(task) => {
            assert_eq!(task.state_data.reviewer.as_str(), "reviewer-for-acme");
        }
        task_machine::State::Published(_) => {}
    }

    Ok(())
}
```

Key details:

- Validator methods run against your persisted type and return either `Ok(...)` for the matching state or `Err(statum::Error::InvalidState)`.
- Machine fields are available by name inside validator methods through generated bindings, so `client` and `name` are usable without boilerplate parameter plumbing.
- Unit states return `statum::Result<()>`; data-bearing states return `statum::Result<StateData>`.
- `.build()` returns the generated wrapper enum, which you can match as `task_machine::State`.
- If any validator is `async`, the generated builder becomes `async`.
- If no validator matches, `.build()` returns `statum::Error::InvalidState`.

Examples: [statum-examples/src/examples/09-persistent-data.rs](statum-examples/src/examples/09-persistent-data.rs), [statum-examples/src/examples/10-persistent-data-vecs.rs](statum-examples/src/examples/10-persistent-data-vecs.rs)

More detail: [docs/persistence-and-validators.md](docs/persistence-and-validators.md)

## Core Rules

`#[state]`

- Apply it to an enum.
- Variants must be unit variants or single-field tuple variants.
- Generics on the state enum are not supported.

`#[machine]`

- Apply it to a struct.
- The first generic parameter must match the `#[state]` enum name.
- Put `#[machine]` above `#[derive(...)]`.

`#[transition]`

- Apply it to `impl Machine<State>` blocks that define legal transitions.
- Transition methods must take `self` or `mut self`.
- Return `Machine<NextState>` directly, or wrap it in `Result` / `Option` when the transition is conditional.
- Use `transition_with(data)` when the target state carries data.

`#[validators]`

- Use `#[validators(Machine)]` on an `impl` block for your persisted type.
- Define one `is_{state}` method per state variant.
- Return `statum::Result<()>` for unit states or `statum::Result<StateData>` for data-bearing states.
- Prefer `into_machine()` for single-item reconstruction.
- For collections in the same module, call `.into_machines()` directly.
- From other modules, import `machine::IntoMachinesExt as _` first.

## When To Use Statum

Use Statum when:

- Workflow order is stable and meaningful.
- Invalid transitions are expensive.
- Available methods should change by phase.
- Some data is only valid in specific states.

Do not use Statum when:

- The workflow is highly ad hoc or user-authored.
- Branching is mostly runtime business logic.
- States are still changing faster than the API around them.

More design guidance: [docs/typestate-builder-design-playbook.md](docs/typestate-builder-design-playbook.md)

## Common Gotchas

**`missing fields marker and state_data`**

Your derives expanded before `#[machine]`. Put `#[machine]` above `#[derive(...)]`.

**Transition helpers in the wrong place**

Keep non-transition helpers in normal `impl` blocks. `#[transition]` is for protocol edges, not general utility methods.

**State shape errors**

`#[state]` accepts unit variants and single-field tuple variants only.

## Learn More

- Examples: [statum-examples/src/examples/](statum-examples/src/examples/)
- Typed rehydration and validators: [docs/persistence-and-validators.md](docs/persistence-and-validators.md)
- Patterns and advanced usage: [docs/patterns.md](docs/patterns.md)
- Typestate builder design playbook: [docs/typestate-builder-design-playbook.md](docs/typestate-builder-design-playbook.md)
- API docs: [docs.rs/statum](https://docs.rs/statum)

## Stability

- Stable Rust is the target.
- MSRV: `1.93`
