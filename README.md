<div align="center">
    <img src="https://github.com/eboody/statum/raw/main/logo.png" alt="statum Logo" width="150">
</div>

# Statum

Statum is a zero-boilerplate library for finite-state machines in Rust, with compile-time state transition validation.

## Why Statum
- Compile-time safety for transitions
- Minimal boilerplate with macros
- Data-bearing states
- Persistence-friendly validation

## Quick Start

```rust
use statum::{machine, state, transition};

#[state]
pub enum LightState {
    Off,
    On,
}

#[machine]
pub struct Light<LightState> {
    name: String,
}

#[transition]
impl Light<Off> {
    fn switch_on(self) -> Light<On> {
        self.transition()
    }
}

#[transition]
impl Light<On> {
    fn switch_off(self) -> Light<Off> {
        self.transition()
    }
}

fn main() {
    let light = Light::<Off>::builder()
        .name("desk lamp".to_owned())
        .build();

    let light = light.switch_on();
    let _light = light.switch_off();
}
```

## State Data

```rust
use statum::{machine, state, transition};

#[state]
pub enum ReviewState {
    Draft,
    InReview(ReviewData),
    Published,
}

pub struct ReviewData {
    reviewer: String,
}

#[machine]
pub struct Document<ReviewState> {
    id: String,
}

#[transition]
impl Document<Draft> {
    fn submit(self, reviewer: String) -> Document<InReview> {
        self.transition_with(ReviewData { reviewer })
    }
}

#[transition]
impl Document<InReview> {
    fn publish(self) -> Document<Published> {
        self.transition()
    }
}
```

## Validators (Persistence)

```rust
use statum::{machine, state, validators};

#[state]
pub enum TaskState {
    Draft,
    InReview(String),
    Published,
}

#[machine]
pub struct Task<TaskState> {
    id: String,
}

pub struct TaskRow {
    status: String,
}

#[validators(Task)]
impl TaskRow {
    fn is_draft(&self) -> Result<(), statum::Error> {
        if self.status == "draft" { Ok(()) } else { Err(statum::Error::InvalidState) }
    }

    fn is_in_review(&self) -> Result<String, statum::Error> {
        if self.status == "review" { Ok("reviewer".into()) } else { Err(statum::Error::InvalidState) }
    }

    fn is_published(&self) -> Result<(), statum::Error> {
        if self.status == "published" { Ok(()) } else { Err(statum::Error::InvalidState) }
    }
}
```

The macro generates:
- `TaskSuperState`, an enum of all possible machine states.
- `machine_builder()` on the data type, returning `Result<TaskSuperState, statum::Error>`.
- A batch builder for processing collections.

## API Rules (Current)

### `#[state]`
- Must be an enum.
- Must have at least one variant.
- Variants must be unit or single-field tuple variants.
- Generics on the enum are not supported.

### `#[machine]`
- Must be a struct.
- First generic parameter must match the `#[state]` enum name.
- Derives on `#[state]` are propagated to generated variant types.
- Prefer `#[machine]` above `#[derive(..)]` to avoid derive ordering surprises.

### `#[transition]`
- Must be applied to `impl Machine<State>` blocks.
- Methods must take `self` or `mut self` as the first argument.
- Return type must be `Machine<NextState>` or `Option<Result<...>>` wrappers.
- Data-bearing states must use `transition_with(data)`.

### `#[validators]`
- Use `#[validators(Machine)]` on an `impl` block for your persistent data type.
- Must define an `is_{state}` method for every state variant (snake_case).
- Each method returns `Result<()>` for unit states or `Result<StateData>` for data states.
- Async validators are supported; if any validator is async, the generated builder is async.
- The macro also generates a `{Machine}SuperState` enum that wraps each concrete machine state, so you can match on a single return type when reconstructing from persistence (a typestate builder pattern).

## Typestate Builder Ergonomics
If you just want a clean, ergonomic builder flow for your own stored data, lean on the generated superstate and a local alias:

```rust
type TaskState = TaskMachineSuperState;

fn rebuild_task(
    row: &TaskRow,
    client: Client,
    db_pool: DbPool,
) -> Result<TaskState, statum::Error> {
    row.machine_builder()
        .client(client)
        .db_pool(db_pool)
        .build()
}

match rebuild_task(&row, client, db_pool)? {
    TaskState::Draft(m) => { /* ... */ }
    TaskState::InReview(m) => { /* ... */ }
    TaskState::Done(m) => { /* ... */ }
}
```

This keeps the typestate builder pattern explicit while avoiding long type names in match arms.

## Examples
See `statum-examples/src/examples/` for the full suite of examples.
