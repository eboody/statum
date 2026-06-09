# State Empty Enum

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_state_empty_enum.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_state_empty_enum.stderr`

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
enum EmptyState {}
```

## Compiler Output

```text
error: Error: `#[state]` enum `EmptyState` must declare at least one variant.
       Found: `enum EmptyState {}`
       Expected: `enum EmptyState { Draft, InReview(InReviewData) }`
       Fix: add at least one unit state or single-payload state variant.
  --> tests/ui/invalid_state_empty_enum.rs:13:6
   |
13 | enum EmptyState {}
   |      ^^^^^^^^^^

error[E0601]: `main` function not found in crate `$CRATE`
  --> tests/ui/invalid_state_empty_enum.rs:13:19
   |
13 | enum EmptyState {}
   |                   ^ consider adding a `main` function to `$DIR/tests/ui/invalid_state_empty_enum.rs`
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

- Found: `enum EmptyState {}`
- Expected: `enum EmptyState { Draft, InReview(InReviewData) }`
- Fix: add at least one unit state or single-payload state variant.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
