# Statum API Surface

This page describes the current public API in the `0.5` line. Use it as the
reference companion to the README:

- README: first-contact pitch and quick start
- This page: generated surface and naming
- `persistence-and-validators.md`: deeper rehydration guide
- `patterns.md`: usage patterns after the basics click

## Core Vocabulary

- `#[state]` defines the legal phases.
- `#[machine]` defines the durable machine context.
- `#[transition]` defines legal edges between phases.
- `#[validators]` rebuilds typed machines from persisted data.
- `statum::projection` reduces event streams into validator rows before rebuilds.

## `#[state]`

### Rules

- Apply it to an enum.
- The enum must have at least one variant.
- Variants must be unit variants or single-field tuple variants.
- Struct variants are not supported.
- Generics on the state enum are not supported.

### Generated Surface

Given:

```rust
#[state]
pub enum DocumentState {
    Draft,
    InReview(ReviewData),
    Published,
}
```

Statum generates:

- One marker type per variant: `Draft`, `InReview`, `Published`
- A state-family trait such as `DocumentStateTrait`
- An uninitialized marker such as `UninitializedDocumentState`
- Implementations of `statum::StateMarker`
- Implementations of `statum::UnitState` or `statum::DataState`

The generated marker type’s associated `Data` is `()` for unit states and the
tuple payload type for data-bearing states.

## `#[machine]`

### Rules

- Apply it to a struct.
- The first generic parameter must match the `#[state]` enum name exactly.
- If the machine derives `Clone`, `Debug`, and so on, the `#[state]` enum must
  derive the same traits.
- If you add derives, place `#[machine]` above `#[derive(...)]`.

### Generated Surface

Statum expands the machine to include hidden state tracking fields:

- `marker: core::marker::PhantomData<S>`
- `state_data: S::Data`

Plus your own machine fields.

It also generates:

- A `builder()` for each concrete state
- A machine-scoped `machine::State` enum for matching rebuilt machines
- A machine-scoped `machine::Fields` struct for per-item batch machine context

Example:

```rust
let draft = Document::<Draft>::builder()
    .id("doc-1".to_owned())
    .build();

let review = Document::<InReview>::builder()
    .id("doc-1".to_owned())
    .state_data(ReviewData {
        reviewer: "alice".to_owned(),
    })
    .build();
```

## `#[transition]`

### Rules

- Apply it to an `impl Machine<CurrentState>` block.
- Transition methods must take `self` or `mut self`.
- Return `Machine<NextState>` directly, or wrappers like
  `Result<Machine<NextState>, E>` or `Option<Machine<NextState>>`.
- `NextState` must be a variant from the machine’s `#[state]` enum.

### Transition Helpers

Inside a `#[transition]` block, use:

- `self.transition()` for unit target states
- `self.transition_with(data)` for data-bearing target states
- `self.transition_map(|current| next_data)` when the next state’s payload is
  built by consuming the current state’s payload

Statum also implements the public advanced traits:

- `statum::CanTransitionTo<Next>`
- `statum::CanTransitionWith<Data>`
- `statum::CanTransitionMap<Next>`

The machine-specific helper traits generated behind the scenes are
implementation details, not supported extension points.

## `#[validators]`

### Attribute Form

Use:

```rust
#[validators(MyMachine)]
impl StoredRow {
    fn is_draft(&self) -> statum::Result<()> { /* ... */ }
    async fn is_in_review(&self) -> statum::Result<ReviewData> { /* ... */ }
}
```

### Rules

- The impl must define at least one validator method.
- There must be one `is_{state}` method per state variant.
- Validator methods must take exactly `&self`.
- Unit states return `statum::Result<()>`.
- Data-bearing states return `statum::Result<StateData>`.
- If any validator is `async`, the generated builders become `async`.

### Generated Surface

Statum generates:

- `into_machine()` for rebuilding one value
- A machine-scoped `my_machine::State` enum for matching rebuilt output
- A machine-scoped `my_machine::Fields` struct for heterogeneous batch context
- A machine-scoped `my_machine::IntoMachinesExt` trait for cross-module batch
  reconstruction
- `.into_machines()` for the shared-machine-fields case
- `.into_machines_by(|row| my_machine::Fields { ... })` for the per-item case

Inside validator bodies, machine fields are available by name through generated
bindings. Persisted-row fields still live on `self`.

## Batch Rehydration Naming

The canonical names are:

- `row.into_machine()`
- `rows.into_machines()`
- `rows.into_machines_by(|row| machine::Fields { ... })`
- `machine::State`

From another module, import the machine-scoped trait first:

```rust
use document_machine::IntoMachinesExt as _;
```

## Projection Helpers

`statum::projection` is the adapter layer for append-only storage and
event-sourced rebuilds.

- `ProjectionReducer<Event>` defines a typed fold
- `reduce_one(events, &reducer)` folds one stream
- `reduce_grouped(events, key_fn, &reducer)` folds interleaved streams grouped
  by key while preserving first-seen key order

Use projection first, then validator rebuilds:

```rust
use statum::projection::{ProjectionReducer, reduce_grouped};

let rows = reduce_grouped(events, |event| event.order_id, &OrderProjector)?;
let machines = rows.into_machines().build();
```

## Legacy Names That Are Gone

These are no longer the supported public surface:

- `TaskMachineSuperState`
- `machine_builder()`
- `machines_builder()`
- public generated helper traits like `TaskMachineTransitionTo`

Use the canonical names from the sections above instead.
