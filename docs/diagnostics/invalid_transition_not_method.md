# Transition Not Method

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_transition_not_method.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_transition_not_method.stderr`

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

use statum_macros::{machine, state, transition};


#[state]
enum State {
    A,
    B,
}

#[machine]
struct Machine<State> {}

#[transition]
impl Machine<A> {
    fn to_b(_value: u64) -> Machine<B> {
        unimplemented!()
    }
}
```

## Compiler Output

```text
error: Error: `#[transition]` method `Machine<A>::to_b` must take `self` or `mut self` as its receiver.
       Found: `fn to_b(...)`
       Expected: `fn to_b(self) -> Machine<NextState>`
       Fix: change the method receiver to `self` or `mut self`.
  --> tests/ui/invalid_transition_not_method.rs:25:5
   |
25 |     fn to_b(_value: u64) -> Machine<B> {
   |     ^^

error[E0601]: `main` function not found in crate `$CRATE`
  --> tests/ui/invalid_transition_not_method.rs:28:2
   |
28 | }
   |  ^ consider adding a `main` function to `$DIR/tests/ui/invalid_transition_not_method.rs`
```

## Corrected Example

```rust
use statum::{machine, state, transition};

#[state]
enum WorkflowState {
    Draft,
    Review,
}

#[machine]
struct WorkflowMachine<WorkflowState> {}

#[transition]
impl WorkflowMachine<Draft> {
    fn submit(self) -> WorkflowMachine<Review> {
        self.transition_to()
    }
}
```

## Explanation

- Found: `fn to_b(...)`
- Expected: `fn to_b(self) -> Machine<NextState>`
- Fix: change the method receiver to `self` or `mut self`.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
