# Migrating to Statum 0.5

This guide is for code written against earlier Statum surfaces. If you are
starting fresh on `0.5`, you can skip this page and start with the README plus
`new-api.md`.

## Migration Checklist

1. Add `#[transition]` to transition impl blocks.
2. Switch validators to `#[validators(Machine)]`.
3. Rename rebuild entrypoints to `into_machine()`, `.into_machines()`, and
   `.into_machines_by(...)`.
4. Match rebuilt output on `machine::State`.
5. Move code that relied on legacy generated helper traits to the crate-level
   advanced traits or to direct machine methods.

## `#[transition]` Is Explicit

Before:

```rust
impl Machine<Draft> {
    fn submit(self) -> Machine<InReview> {
        self.transition()
    }
}
```

After:

```rust
#[transition]
impl Machine<Draft> {
    fn submit(self) -> Machine<InReview> {
        self.transition()
    }
}
```

## `#[validators]` Uses the Machine Name Only

Before:

```rust
#[validators(state = TaskState, machine = TaskMachine)]
impl StoredTask {
    fn is_draft(&self) -> Result<()> { /* ... */ }
}
```

After:

```rust
#[validators(TaskMachine)]
impl StoredTask {
    fn is_draft(&self) -> statum::Result<()> { /* ... */ }
}
```

Statum resolves the state family from the machine definition.

## Validator Expectations Are Stricter

- You need one `is_{state}` method per state variant.
- Validator methods must take exactly `&self`.
- Unit states return `statum::Result<()>`.
- Data-bearing states return `statum::Result<StateData>`.
- If any validator is `async`, the generated builders become `async`.

## Canonical Rebuild Names Changed

Use these names in `0.5`:

- `into_machine()` for one item
- `.into_machines()` when machine fields are shared across the collection
- `.into_machines_by(|row| machine::Fields { ... })` when machine fields vary
  per item
- `machine::State` when matching rebuilt output

Removed names:

- `machine_builder()`
- `machines_builder()`
- `TaskMachineSuperState`

Cross-module batch rebuilds still need the machine-scoped trait import:

```rust
use task_machine::IntoMachinesExt as _;
```

## Legacy Generated Traits Are Gone

The old public generated helper traits, such as `TaskMachineTransitionTo` or
`StateVariant`, are no longer the supported API.

Use:

- direct machine methods
- `statum::StateMarker`
- `statum::UnitState`
- `statum::DataState`
- `statum::CanTransitionTo<Next>`
- `statum::CanTransitionWith<Data>`
- `statum::CanTransitionMap<Next>`

## State and Machine Definitions Are Checked More Strictly

### `#[state]`

- Must be an enum
- Must have at least one variant
- Variants must be unit or single-field tuple variants
- Struct variants and generics are rejected

### `#[machine]`

- Must be a struct
- The first generic parameter must match the `#[state]` enum name
- Matching derives on the `#[state]` enum and machine are required when needed
- `#[machine]` should sit above `#[derive(...)]`

## Construction Is Builder-First

Use the generated per-state builders:

```rust
let draft = Machine::<Draft>::builder()
    .field_a(...)
    .build();

let review = Machine::<InReview>::builder()
    .field_a(...)
    .state_data(ReviewData { ... })
    .build();
```

You can still use the generated positional constructor if you want it, but the
builder is the intended path.

## New Additive Helpers

`0.5` also adds surface area you may want during migration:

- `.into_machines_by(...)` for heterogeneous batch machine context
- `statum::projection::{ProjectionReducer, reduce_one, reduce_grouped}` for
  event-log projection before rehydration
- `transition_map(...)` for data-to-data transitions that should consume the
  current state payload

## Examples Moved

- Old `statum/examples/*.rs` examples are gone.
- Toy demos now live under `statum-examples/src/toy_demos/`.
- Service-shaped and protocol-shaped showcases live under
  `statum-examples/src/showcases/`.

## Recommended Migration Order

1. Update the `#[state]` enum to match the current variant rules.
2. Update the machine generic and derive placement.
3. Add `#[transition]` to protocol-edge impl blocks.
4. Rename validators and rehydration entrypoints.
5. Update match sites to use `machine::State`.
6. Move any generic helper code off removed legacy traits.
7. Re-run `cargo test -p statum-macros` and your workspace tests.
