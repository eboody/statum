# Transition Introspect Unknown Key

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_transition_introspect_unknown_key.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_transition_introspect_unknown_key.stderr`

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
    #[introspect(result = WorkflowMachine<Review>)]
    fn submit(self) -> WorkflowMachine<Review> {
        self.transition()
    }
}

fn main() {}
```

## Compiler Output

```text
error: Error: unknown `#[introspect(...)]` key `result`.
       Found: `result = ...`
       Expected: `return = WorkflowMachine<NextState>`
       Fix: use the `return` key or remove the extra entry.
  --> tests/ui/invalid_transition_introspect_unknown_key.rs:24:18
   |
24 |     #[introspect(result = WorkflowMachine<Review>)]
   |                  ^^^^^^
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

- Found: `result = ...`
- Expected: `return = WorkflowMachine<NextState>`
- Fix: use the `return` key or remove the extra entry.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
