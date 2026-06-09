# Typed Rehydration with Validators

`#[validators]` is Statum's typed rehydration feature. Use it when you need to turn a row, document, event payload, or other persisted representation back into a typed machine.

This is the boundary where raw persisted facts either become one legal typed
state or stay invalid runtime data. That is how Statum keeps not-yet-validated
states out of ordinary code.

## Mental Model

You define:

- A `#[state]` enum that names the legal phases.
- A `#[machine]` struct that carries durable context.
- A persisted type, such as `DbRow` or `StoredTask`.
- One validator method per state variant on that persisted type.

Statum generates:

- `TaskMachine::rebuild(&row)` and `into_machine()` for rebuilding one machine.
- A machine-scoped enum like `task_machine::SomeState`.
  `task_machine::State` remains an alias for compatibility.
- A machine-scoped `task_machine::Fields` struct for heterogeneous batch reconstruction.
- A machine-scoped batch trait like `task_machine::IntoMachinesExt`.

The important part is what Statum does not generate: it does not treat stored
data as already trustworthy. Validators decide whether the persisted value
actually represents `Draft`, `InReview`, `Published`, or nothing legal at all.

## Pick The Right Entry Point

For the public vocabulary around `project`, `rebuild`, `rehydrate`, `recover`,
`explain`, and unchecked escape hatches, see
[rehydration-vocabulary.md](rehydration-vocabulary.md). For the grep-able audit
catalog of current and reserved escape hatches, see
[escape-hatches.md](escape-hatches.md).

Use:

- `TaskMachine::rebuild(&row)` when rebuilding one persisted value
- `into_machine()` as the fallback single-item entrypoint
- `TaskMachine::rebuild_many(rows)` or `.into_machines()` when every item shares the same machine fields
- `.into_machines_by(|row| task_machine::Fields { ... })` when each item needs
  different machine fields

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

fn rebuild(row: &DbRow) -> statum::Result<task_machine::SomeState> {
    TaskMachine::rebuild(row)
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
    task_machine::SomeState::Draft(machine) => {}
    task_machine::SomeState::InReview(machine) => {
        assert_eq!(machine.state_data.reviewer.as_str(), "reviewer-for-acme");
    }
    task_machine::SomeState::Published(machine) => {}
}
```

After that match, you are no longer carrying "a row plus a status field." You
are carrying one explicit legal state.

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

That is how typed rehydration can fetch extra data or use shared context without manual parameter threading. Persisted-row fields are not rebound: keep reading them from `self.status`, `self.id`, and so on.

## Return Types

- Unit state: `statum::Result<()>` or `statum::Validation<()>`
- Data-bearing state: `statum::Result<StateData>` or `statum::Validation<StateData>`

Example:

- `Draft` -> `statum::Result<()>`
- `InReview(ReviewData)` -> `statum::Result<ReviewData>`

Use `statum::Result<T>` when you only care whether the row matched that state.
Use `statum::Validation<T>` when a failed match should carry a stable
`reason_key` and optional message into rebuild reports.

`Result<T, statum::Rejection>` is also supported directly when you want the
same diagnostic surface without the alias.
Prefer `Validation<T>` as the stable shape for diagnostic validators; renamed
rejection aliases are not syntax-recognized for report details today.

If every validator returns `Err(statum::Error::InvalidState)` or a diagnostic
rejection, reconstruction still fails with `InvalidState`.

## Rebuild Reports and Explain Mode

Enable the `rebuild-reports` Cargo feature to use `.build_report()` for one row
when you want the rebuild result plus the evaluation trace that produced it,
while preserving `.build()` semantics. These report builders stop at the first
accepted candidate and mark `RebuildReport.ambiguity` as `NotChecked`.
Collection `.build_reports()` also requires `rebuild-batch`.

Use `.explain()` on a single-row builder when you are debugging persisted data
and need every candidate state evaluated even after one candidate accepts. The
returned `RebuildReport` is non-throwing: invalid rows return
`Err(statum::Error::InvalidState)` in `report.result`, while
`report.attempts` keeps the per-candidate accepted/rejected evidence.

- `RebuildAttempt.matched` tells you which validators accepted the row.
- `RebuildAttempt.reason_key` and `RebuildAttempt.message` are populated only
  for diagnostic validators.
- `.explain()` sets `RebuildReport.ambiguity` to `Unambiguous` when zero or one
  candidates accepted, or `Ambiguous { matched_states }` when more than one
  candidate accepted.
- `.into_result()` keeps the normal rebuild result surface, so callers can opt
  into reports without changing success-path handling.

For example, an admin repair tool can inspect every candidate for a bad row:

```rust
let report = TaskMachine::rebuild(&row)
    .client("acme".to_owned())
    .name("spec".to_owned())
    .explain();

for attempt in &report.attempts {
    eprintln!(
        "candidate={} accepted={} reason={:?}",
        attempt.target_state,
        attempt.matched,
        attempt.reason_key,
    );
}
```

## Persisted-Row Test Fixtures

Use `statum::testing::rehydrate::row_fixture` when a test fixture is a row that
should rebuild into a named state, or when a bad row should fail with specific
report evidence. The helper observes the `RebuildReport` you pass in; it does
not rerun validators or claim the database is complete.

```rust
use statum::testing::rehydrate::row_fixture;

#[test]
fn persisted_row_rebuilds_into_review_state() {
    let row = DbRow {
        status: Status::InReview,
    };

    let report = TaskMachine::rebuild(&row)
        .client("acme".to_owned())
        .name("spec".to_owned())
        .build_report();

    row_fixture(report)
        .rebuilds_as("InReview")
        .matched_by("is_in_review");
}

#[test]
fn persisted_row_failure_reports_missing_reviewer() {
    let row = DbRow {
        status: Status::InReview,
    };

    let report = TaskMachine::rebuild(&row)
        .client("acme".to_owned())
        .name("spec".to_owned())
        .explain();

    row_fixture(report)
        .fails()
        .candidate_states(["Draft", "InReview", "Published"])
        .unambiguous()
        .rejected_by("is_in_review", "missing_reviewer");
}
```

Use `snapshot_fixture(report)` for serialized document snapshots and
`event_fixture(report)` for event-log projection reports when the same report
assertions make the fixture intent clearer.

## Async Validators

If any validator is `async`, the generated builder becomes `async` too:

```rust
let machine = row
    .into_machine()
    .client("acme".to_owned())
    .build()
    .await?;
```

The async form also works through `TaskMachine::rebuild(&row)`.

This is useful when typed rehydration requires a network call or a database fetch.

Example: [../statum-examples/src/toy_demos/09-persistent-data.rs](../statum-examples/src/toy_demos/09-persistent-data.rs)

## Batch Reconstruction

Enable the `rebuild-batch` Cargo feature before using collection rebuild
helpers.

For collections in the same module as the `#[validators]` impl, `TaskMachine::rebuild_many(rows)` is the type-first batch entrypoint and `.into_machines()` is the fallback:

```rust
let machines = TaskMachine::rebuild_many(rows)
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

If each row carries its own machine context, use `.into_machines_by(...)` and return the generated `machine::Fields` struct:

```rust
use task_machine::IntoMachinesExt as _;

let machines = rows
    .into_machines_by(|row| task_machine::Fields {
        client: row.client.clone(),
        name: row.name.clone(),
    })
    .build()
    .await;
```

This returns a collection of per-item results, which lets you decide whether to fail fast, collect only valid machines, or report partial errors.

In other words, batch rebuilds preserve per-item failure information instead of
forcing one all-or-nothing result shape. The ordering contract is index-based:
output slot `i` corresponds to input slot `i`, including async validators and
`.into_machines_by(...)` per-row fields. Use `.build_reports()` when you need a
lossless vector of per-row successes/failures plus validator evidence, then build
any aggregate counts from that vector.

For the batch helper contract, partial-failure policies, and an aggregate summary
API sketch, see [batch-rehydration-design.md](batch-rehydration-design.md).

Examples: [../statum-examples/src/toy_demos/10-persistent-data-vecs.rs](../statum-examples/src/toy_demos/10-persistent-data-vecs.rs), [../statum-examples/src/toy_demos/14-batch-machine-fields.rs](../statum-examples/src/toy_demos/14-batch-machine-fields.rs)

## Event Logs: Project First, Rehydrate Second

`#[validators]` works on one persisted shape at a time. For append-only event logs, project the stream into a row-like snapshot first, then rebuild typed machines from that projection.

Statum ships small projection helpers for that layer:

```rust
use statum::projection::{ProjectionReducer, reduce_grouped};

let projections = reduce_grouped(events, |event| event.order_id, &OrderProjector)?;
let machines = projections
    .into_machines()
    .build();
```

`ProjectionReducer` gives you a typed fold, `reduce_one(...)` handles a single stream, and `reduce_grouped(...)` handles interleaved streams keyed by something like `order_id` while preserving first-seen key order.

Example: [../statum-examples/src/showcases/sqlite_event_log_rebuild.rs](../statum-examples/src/showcases/sqlite_event_log_rebuild.rs)

## Integration Boundaries: SQLx, Serde, and Axum

Statum does not replace your storage, JSON, or HTTP stack. Keep those tools at
the edge and make typed rehydration the handoff into workflow code.

- SQLx owns fetching and updating rows. In
  [axum_sqlite_review.rs](../statum-examples/src/showcases/axum_sqlite_review.rs),
  `fetch_document_row(...)` returns a plain `DocumentRow`; `load_document_state(...)`
  is the boundary that rebuilds that row into `document_machine::SomeState`.
- Axum owns routing and request/response serialization. Handlers validate input,
  fetch the row, rebuild the typed state, call only legal transition methods, and
  then persist the resulting machine data before returning JSON.
- Serde owns snapshots crossing a JSON boundary. In
  [serde_json_snapshot.rs](../statum-examples/src/showcases/serde_json_snapshot.rs),
  the store deserializes a `CartSnapshot`, rebuilds a typed cart machine, applies
  `checkout(...)` only when the snapshot is `Open`, then serializes the updated
  snapshot back to storage.

The pattern is the same across integrations:

```text
row / JSON / request edge -> rebuild typed machine -> legal transition -> persist / return
```

That boundary is deliberately explicit. Invalid persisted facts stay as edge
data and become an error; they do not enter ordinary workflow code as a typed
machine.

## Failure Model

- A validator that matches returns `Ok(...)` and selects that state.
- A validator that does not match should return `Err(statum::Error::InvalidState)` or a diagnostic `Err(statum::Rejection { .. })`.
- Reconstruction fails when no validator matches.
- Batch reconstruction returns one result per item, so callers can decide
  whether to stop on the first invalid row or collect partial successes.

Keep validators narrowly focused on state membership. Put cross-cutting orchestration around the rebuild call, not inside every validator.
