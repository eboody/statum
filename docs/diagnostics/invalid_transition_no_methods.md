# Transition No Methods

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_transition_no_methods.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_transition_no_methods.stderr`

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
impl Machine<A> {}
```

## Compiler Output

```text
error: Error: `#[transition]` impl for `Machine<A>` must contain at least one transition method.
       Found: `impl Machine<A> {}`
       Expected: `fn submit(self) -> Machine<NextState>` or a supported wrapper around that same machine path
       Fix: add at least one method that consumes `self` and returns the next `#[machine]` state.
  --> tests/ui/invalid_transition_no_methods.rs:24:6
   |
24 | impl Machine<A> {}
   |      ^^^^^^^

error[E0601]: `main` function not found in crate `$CRATE`
  --> tests/ui/invalid_transition_no_methods.rs:24:19
   |
24 | impl Machine<A> {}
   |                   ^ consider adding a `main` function to `$DIR/tests/ui/invalid_transition_no_methods.rs`
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

- Found: `impl Machine<A> {}`
- Expected: `fn submit(self) -> Machine<NextState>` or a supported wrapper around that same machine path
- Fix: add at least one method that consumes `self` and returns the next `#[machine]` state.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
