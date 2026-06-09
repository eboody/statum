# State Attr Args

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_state_attr_args.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_state_attr_args.stderr`

## Broken Example

```rust
#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::state;

#[state(start = Draft)]
enum WorkflowState {
    Draft,
}
```

## Compiler Output

```text
error: Error: `#[state]` does not accept arguments.
       Found: `#[state(start = Draft)]`
       Expected: `#[state] enum WorkflowState { Draft, Review(ReviewData) }`
       Fix: remove the attribute arguments and describe states with enum variants instead.
  --> tests/ui/invalid_state_attr_args.rs:13:1
   |
13 | #[state(start = Draft)]
   | ^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: this error originates in the attribute macro `state` (in Nightly builds, run with -Z macro-backtrace for more info)

error[E0601]: `main` function not found in crate `$CRATE`
  --> tests/ui/invalid_state_attr_args.rs:16:2
   |
16 | }
   |  ^ consider adding a `main` function to `$DIR/tests/ui/invalid_state_attr_args.rs`
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

- Found: `#[state(start = Draft)]`
- Expected: `#[state] enum WorkflowState { Draft, Review(ReviewData) }`
- Fix: remove the attribute arguments and describe states with enum variants instead.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
