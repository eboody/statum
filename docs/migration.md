# Migrating to Statum 0.6

This guide starts with the `0.6` breaking change for `0.5.x` users, then keeps
the earlier `0.5` migration notes for older code. If you are starting fresh on
`0.6`, you can skip this page and start with the README plus
`tutorial-review-workflow.md`.

## 0.6 Checklist

1. Remove any imports of `statum::bon` or `statum::bon::builder`.
2. If you still want bon in your application, depend on `bon` directly.
3. Keep using `Machine::<State>::builder()`, `into_machine()`,
   `.into_machines()`, and `.into_machines_by(...)`; those call shapes are
   unchanged.
4. Treat generated builder internals as implementation details rather than
   stable public API.

## `statum::bon` Is Gone

Before:

```rust
use statum::bon;
use statum::bon::builder as _;
```

After:

```toml
[dependencies]
bon = "3"
```

Or just remove the import if you were only using Statum's generated builders.

`0.6` keeps the builder-first workflow surface, but Statum now owns the builder
implementation instead of re-exporting bon. The supported entry points stay the
same:

- `Machine::<State>::builder()`
- `row.into_machine()`
- `rows.into_machines()`
- `rows.into_machines_by(...)`

The intentional break is the bon re-export and any bon-specific generated
builder internals, not the normal Statum call patterns.

## Earlier 0.5 Migration Checklist

1. Add `#[transition]` to transition impl blocks.
2. Switch validators to `#[validators(Machine)]`.
3. Rename rebuild entrypoints to `into_machine()`, `.into_machines()`, and
   `.into_machines_by(...)`.
4. Match rebuilt output on `machine::SomeState`.
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

Use these names in `0.5` and later:

- `into_machine()` for one item
- `.into_machines()` when machine fields are shared across the collection
- `.into_machines_by(|row| machine::Fields { ... })` when machine fields vary
  per item
- `machine::SomeState` when matching rebuilt output
- `machine::State` is still available as a compatibility alias during migration

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

`0.5` and later also add surface area you may want during migration:

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
5. Update match sites to use `machine::SomeState`.
6. Move any generic helper code off removed legacy traits.
7. Re-run `cargo test -p statum-macros` and your workspace tests.
