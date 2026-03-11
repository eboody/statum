# Statum New API (Inferred from Code)

This document is inferred from the current macros and core crates. It reflects how the code behaves today, not necessarily the final public contract. If this diverges from the intended design, treat this as a starting point for edits.

## Core Concepts
- `#[state]` defines the state enum and generates per-variant marker types plus a trait bound.
- `#[machine]` defines the machine struct and injects state tracking fields.
- `#[transition]` decorates `impl` blocks containing transition methods.
- `#[validators]` decorates an `impl` block for persistent data, generating helpers to build a machine from stored data.

## `#[state]` Macro
### Rules
- Must be applied to an enum.
- Must have at least one variant.
- Variants must be either:
  - unit variants, or
  - single-field tuple variants (e.g. `InReview(ReviewData)`).
- Struct variants are not allowed.
- Generics on the state enum are not supported.

### Generated Items
Given:

```rust
#[state]
pub enum DocumentState {
    Draft,
    InReview(ReviewData),
    Published,
}
```

The macro generates:
- A trait named `DocumentStateTrait` with an associated type `Data`.
- One struct per variant: `Draft`, `InReview(pub ReviewData)`, `Published`.
- An uninitialized state marker: `UninitializedDocumentState`.
- Implementations of the public advanced traits:
  - `statum::StateMarker`
  - `statum::UnitState` or `statum::DataState`

## `#[machine]` Macro
### Rules
- Must be applied to a struct.
- The first generic parameter must match the `#[state]` enum name exactly.
- Derives placed on the machine struct must also be present on the `#[state]` enum.

### Generated Fields
The machine struct is expanded to include:
- `marker: core::marker::PhantomData<S>`
- `state_data: S::Data`

Plus any user-defined fields.

### Constructors / Builders
For each state variant, the macro generates a `builder()` method using `bon`:

```rust
let machine = Machine::<Draft>::builder()
    .field_a(...)
    .field_b(...)
    .build();

let machine = Machine::<InReview>::builder()
    .field_a(...)
    .state_data(ReviewData { ... })
    .build();
```

For data-bearing variants, the builder exposes `state_data(..)`; for unit variants it does not.

## Transition Helpers and `#[transition]`
The macro generates internal helper traits so transition methods can call
`self.transition()` or `self.transition_with(data)` inside `#[transition]`
impls. Those helper traits are implementation details, not public API.

It also implements the public advanced traits:
- `statum::CanTransitionTo<Next>`
- `statum::CanTransitionWith<Data>`
- `statum::CanTransitionMap<Next>`

### `#[transition]` Rules
- Must be applied to an `impl Machine<CurrentState>` block.
- Each transition method must be a method (first arg is `self` or `mut self`).
- Return type must be parseable as `Machine<NextState>` or wrappers like `Option<Machine<NextState>>` or `Result<Machine<NextState>, E>`.
- The `NextState` must be a variant of the `#[state]` enum.

### Codegen Behavior
- If `NextState` carries data, the generated implementation expects `transition_with(data)`.
- If `NextState` is unit, the generated implementation expects `transition()`.
- If the current state's payload should be transformed into the next state's payload, use `transition_map(|current| NextData { ... })`.

## Validators (`#[validators]`)
### Attribute Form
Inferred usage is:

```rust
#[validators(MyMachine)]
impl MyPersistentData {
    fn is_draft(&self) -> Result<()> { ... }
    async fn is_in_review(&self) -> Result<ReviewData> { ... }
}
```

### Rules
- The `impl` block must contain at least one `is_*` method.
- There must be an `is_{state}` method for every state variant (snake_case).
- Each `is_*` method must:
  - take exactly `&self` (additional params are injected internally),
  - return `Result<()>` for unit states,
  - return `Result<StateData>` for data states,
  - may be `async` (if any validator is async, generated builders are async).

### Generated Items
- A machine-scoped enum `my_machine::State` with variants for each state, each wrapping `MyMachine<State>`.
- A machine-scoped `my_machine::Fields` struct mirroring the user-defined machine fields.
- An `into_machine()` builder on the persistent data type that returns `Result<my_machine::State, statum::Error>`.
- A machine-scoped `my_machine::IntoMachinesExt` trait for cross-module batch reconstruction with `.into_machines()`.
- The same batch trait also exposes `.into_machines_by(|item| my_machine::Fields { ... })` for per-item machine context.
- A same-module import so `.into_machines()` works directly next to the `#[validators]` impl.

## Projection Helpers

`statum::projection` is the small helper layer for event-sourced or append-only persistence before validator rehydration.

- `ProjectionReducer<Event>` defines a typed fold with `seed` and `apply`.
- `reduce_one(events, &reducer)` folds one stream into one projection.
- `reduce_grouped(events, key_fn, &reducer)` folds interleaved streams into grouped projections while preserving first-seen key order.

This keeps `#[validators]` focused on "projection row -> typed machine" instead of trying to consume raw event streams directly.

## Quick Example (Inferred)
```rust
use statum::{machine, state, transition, validators};

#[state]
pub enum DocState {
    Draft,
    InReview(ReviewData),
    Published,
}

pub struct ReviewData {
    reviewer: String,
}

#[machine]
pub struct Doc<DocState> {
    id: String,
}

#[transition]
impl Doc<Draft> {
    fn submit(self, reviewer: String) -> Doc<InReview> {
        self.transition_with(ReviewData { reviewer })
    }
}

#[transition]
impl Doc<InReview> {
    fn publish(self) -> Doc<Published> {
        self.transition()
    }
}

#[validators(Doc)]
impl StoredDoc {
    fn is_draft(&self) -> Result<()> { Ok(()) }
    fn is_in_review(&self) -> Result<ReviewData> { Ok(ReviewData { reviewer: "a".into() }) }
    fn is_published(&self) -> Result<()> { Ok(()) }
}
```
