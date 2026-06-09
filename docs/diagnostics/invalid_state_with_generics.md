# State With Generics

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_state_with_generics.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_state_with_generics.stderr`

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
enum GenericState<'a, T> {
    Draft(&'a T),
    InProgress(T),
}
```

## Compiler Output

```text
error: Error: `#[state]` enum `GenericState` cannot declare generics.
       Found: `enum GenericState<'a, T> { ... }`
       Expected: `enum GenericState { Draft, Review(ReviewData) }`
       Fix: keep `GenericState` non-generic and move the generic data into a payload type such as `ReviewData<T>`.
  --> tests/ui/invalid_state_with_generics.rs:13:18
   |
13 | enum GenericState<'a, T> {
   |                  ^^^^^^^

error[E0601]: `main` function not found in crate `$CRATE`
  --> tests/ui/invalid_state_with_generics.rs:16:2
   |
16 | }
   |  ^ consider adding a `main` function to `$DIR/tests/ui/invalid_state_with_generics.rs`
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

- Found: `enum GenericState<'a, T> { ... }`
- Expected: `enum GenericState { Draft, Review(ReviewData) }`
- Fix: keep `GenericState` non-generic and move the generic data into a payload type such as `ReviewData<T>`.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
