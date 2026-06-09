# Transition Introspect Missing Return

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_transition_introspect_missing_return.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_transition_introspect_missing_return.stderr`

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

use statum_macros::{machine, state, transition};

#[state]
enum WorkflowState {
    Draft,
    Review,
}

#[machine]
struct WorkflowMachine<WorkflowState> {}

#[transition]
impl WorkflowMachine<Draft> {
    #[introspect]
    fn submit(self) -> WorkflowMachine<Review> {
        self.transition()
    }
}

fn main() {}
```

## Compiler Output

```text
error: Error: `#[introspect(...)]` requires parentheses.
       Found: `#[introspect]`
       Expected: `#[introspect(return = WorkflowMachine<NextState>)]`
       Fix: write `#[introspect(return = ...)]` on the transition method.
  --> tests/ui/invalid_transition_introspect_missing_return.rs:24:5
   |
24 |     #[introspect]
   |     ^
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

- Found: `#[introspect]`
- Expected: `#[introspect(return = WorkflowMachine<NextState>)]`
- Fix: write `#[introspect(return = ...)]` on the transition method.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
