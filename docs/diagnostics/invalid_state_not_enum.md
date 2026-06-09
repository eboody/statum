# State Not Enum

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_state_not_enum.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_state_not_enum.stderr`

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
struct NotAnEnum {
    value: u32,
}
```

## Compiler Output

```text
error: Error: #[state] must be applied to an enum.
       Found: `struct NotAnEnum { ... }`
       Expected: `enum NotAnEnum { Draft, InReview(InReviewData) }`
       Fix: change `NotAnEnum` from a struct into a `#[state]` enum, or remove `#[state]`.
  --> tests/ui/invalid_state_not_enum.rs:13:8
   |
13 | struct NotAnEnum {
   |        ^^^^^^^^^

error[E0601]: `main` function not found in crate `$CRATE`
  --> tests/ui/invalid_state_not_enum.rs:15:2
   |
15 | }
   |  ^ consider adding a `main` function to `$DIR/tests/ui/invalid_state_not_enum.rs`
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

- Found: `struct NotAnEnum { ... }`
- Expected: `enum NotAnEnum { Draft, InReview(InReviewData) }`
- Fix: change `NotAnEnum` from a struct into a `#[state]` enum, or remove `#[state]`.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
