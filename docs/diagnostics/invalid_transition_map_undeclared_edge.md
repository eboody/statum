# Transition Map Undeclared Edge

Status: tracked compiler-fallback placeholder

Source fixture: `../../statum-macros/tests/ui/invalid_transition_map_undeclared_edge.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_transition_map_undeclared_edge.stderr`

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
pub enum WorkflowState {
    Draft(DraftData),
    Review(ReviewData),
    Published,
}

pub struct DraftData {
    title: String,
}

pub struct ReviewData {
    title: String,
    reviewer: String,
}

#[machine]
pub struct WorkflowMachine<WorkflowState> {}

#[transition]
impl WorkflowMachine<Draft> {
    fn start_review(self, reviewer: String) -> WorkflowMachine<Review> {
        self.transition_map(|draft| ReviewData {
            title: draft.title,
            reviewer,
        })
    }
}

#[transition]
impl WorkflowMachine<Review> {
    fn publish(self) -> WorkflowMachine<Published> {
        self.transition()
    }
}

fn main() {
    let review = WorkflowMachine::<Review>::builder()
        .state_data(ReviewData {
            title: "Spec".to_string(),
            reviewer: "ada".to_string(),
        })
        .build();

    let _ = review.transition_map(|data| data);
}
```

## Compiler Output

```text
error[E0277]: the trait bound `WorkflowMachine<Review>: DeclaredTransitionMapEdge<_>` is not satisfied
  --> tests/ui/invalid_transition_map_undeclared_edge.rs:59:20
   |
59 |     let _ = review.transition_map(|data| data);
   |                    ^^^^^^^^^^^^^^ unsatisfied trait bound
   |
help: the trait `DeclaredTransitionMapEdge<_>` is not implemented for `WorkflowMachine<Review>`
      but trait `DeclaredTransitionMapEdge<Review>` is implemented for `WorkflowMachine<Draft>`
  --> tests/ui/invalid_transition_map_undeclared_edge.rs:34:1
   |
34 | #[transition]
   | ^^^^^^^^^^^^^
   = help: for that trait implementation, expected `Draft`, found `Review`
   = note: this error originates in the attribute macro `transition` (in Nightly builds, run with -Z macro-backtrace for more info)
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
