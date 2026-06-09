# Transition Unknown Source State

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_transition_unknown_source_state.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_transition_unknown_source_state.stderr`

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

struct Ghost;

#[transition]
impl Machine<Ghost> {
    fn to_b(self) -> Machine<B> {
        self.transition()
    }
}

fn main() {}
```

## Compiler Output

```text
error: Error: source state `Ghost` in `#[transition]` target `Machine<Ghost>` is not a variant of `#[state]` enum `State`.
       Valid states for `Machine` are: A, B.
       Help: change the impl target to `impl Machine<ValidState>` using one of those variants.
  --> tests/ui/invalid_transition_unknown_source_state.rs:26:14
   |
26 | impl Machine<Ghost> {
   |              ^^^^^
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

- This fixture intentionally records a native Rust compiler error that protects a generated surface or removed legacy API.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
