# Typed Rehydration with Validators

`#[validators]` is Statum's typed rehydration feature. Use it when you need to turn a row, document, event payload, or other persisted representation back into a typed machine.

## Mental Model

You define:

- A `#[state]` enum that names the legal phases.
- A `#[machine]` struct that carries durable context.
- A persisted type, such as `DbRow` or `StoredTask`.
- One validator method per state variant on that persisted type.

Statum generates:

- `into_machine()` for rebuilding one machine.
- A machine-scoped enum like `task_machine::State`.
- A machine-scoped batch trait like `task_machine::IntoMachinesExt`.

## Single-Item Reconstruction

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
        if matches!(self.status, Status::Draft) {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_in_review(&self) -> statum::Result<ReviewData> {
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

fn rebuild(row: &DbRow) -> statum::Result<task_machine::State> {
    row.into_machine()
        .client("acme".to_owned())
        .name("spec".to_owned())
        .build()
}
```

The returned value is a wrapper enum, so you match once and then work with the concrete typed machine:

```rust
let row = DbRow {
    status: Status::InReview,
};

match rebuild(&row)? {
    task_machine::State::Draft(machine) => {}
    task_machine::State::InReview(machine) => {
        assert_eq!(machine.state_data.reviewer.as_str(), "reviewer-for-acme");
    }
    task_machine::State::Published(machine) => {}
}
```

## What Is Available Inside Validator Methods

Validator methods always receive `&self` for the persisted type.

Statum also makes machine fields available by name inside the validator body through generated bindings. If your machine has:

```rust
#[machine]
struct TaskMachine<TaskState> {
    client: String,
    name: String,
}
```

then `client` and `name` are available inside `is_draft`, `is_in_review`, and `is_published`.

That is how typed rehydration can fetch extra data or use shared context without manual parameter threading.

## Return Types

- Unit state: `statum::Result<()>`
- Data-bearing state: `statum::Result<StateData>`

Example:

- `Draft` -> `statum::Result<()>`
- `InReview(ReviewData)` -> `statum::Result<ReviewData>`

If every validator returns `Err(statum::Error::InvalidState)`, reconstruction fails with `InvalidState`.

## Async Validators

If any validator is `async`, the generated builder becomes `async` too:

```rust
let machine = row
    .into_machine()
    .client("acme".to_owned())
    .build()
    .await?;
```

This is useful when typed rehydration requires a network call or a database fetch.

Example: [../statum-examples/src/examples/09-persistent-data.rs](../statum-examples/src/examples/09-persistent-data.rs)

## Batch Reconstruction

For collections in the same module as the `#[validators]` impl, `.into_machines()` works directly:

```rust
let machines = rows
    .into_machines()
    .client("acme".to_owned())
    .build()
    .await;
```

From other modules, import the machine-scoped batch trait first:

```rust
use task_machine::IntoMachinesExt as _;

let machines = rows
    .into_machines()
    .client("acme".to_owned())
    .build()
    .await;
```

This returns a collection of per-item results, which lets you decide whether to fail fast, collect only valid machines, or report partial errors.

Example: [../statum-examples/src/examples/10-persistent-data-vecs.rs](../statum-examples/src/examples/10-persistent-data-vecs.rs)

## Failure Model

- A validator that matches returns `Ok(...)` and selects that state.
- A validator that does not match should return `Err(statum::Error::InvalidState)`.
- Reconstruction fails when no validator matches.

Keep validators narrowly focused on state membership. Put cross-cutting orchestration around the rebuild call, not inside every validator.
