# State Tuple Variant

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_state_tuple_variant.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_state_tuple_variant.stderr`

## Broken Example

```rust
#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport, StateDescriptor, StateMarker,
    TransitionDescriptor, UnitState,
};
use statum_macros::state;

#[state]
enum BadState {
    Draft(u32, u32),
}
```

## Compiler Output

```text
error: Error: `#[state]` enum `BadState` variant `Draft` carries 2 fields, but Statum supports at most one payload type per state.
       Found: `Draft(u32, u32)`
       Expected: `Draft(DraftData)`
       Fix: wrap the current fields in a payload type like `struct DraftData { ... }` and use `enum BadState { Draft(DraftData) }`.
  --> tests/ui/invalid_state_tuple_variant.rs:14:5
   |
14 |     Draft(u32, u32),
   |     ^^^^^^^^^^^^^^^

error[E0601]: `main` function not found in crate `$CRATE`
  --> tests/ui/invalid_state_tuple_variant.rs:15:2
   |
15 | }
   |  ^ consider adding a `main` function to `$DIR/tests/ui/invalid_state_tuple_variant.rs`
```

## Corrected Example

```rust
use statum::state;

#[state]
enum WorkflowState {
    Draft,
    Review(ReviewData),
}

struct ReviewData {
    priority: u8,
}
```

## Explanation

- Found: `Draft(u32, u32)`
- Expected: `Draft(DraftData)`
- Fix: wrap the current fields in a payload type like `struct DraftData { ... }` and use `enum BadState { Draft(DraftData) }`.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
