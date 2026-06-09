# Transition Include Ambiguous Machine Name

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_transition_include_ambiguous_machine_name.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_transition_include_ambiguous_machine_name.stderr`

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

mod beta {
    use super::*;

    #[state]
    enum FlowState {
        Start,
        Done,
    }

    #[machine]
    struct FlowMachine<FlowState> {}
}

mod alpha {
    use super::*;

    #[state]
    enum FlowState {
        Start,
        Done,
    }

    #[machine]
    struct FlowMachine<FlowState> {}

    include!("support/ambiguous_transition_include.rs");
}

fn main() {}
```

## Compiler Output

```text
error: Error: include-generated `#[transition]` impl for `FlowMachine` could not resolve a unique `#[machine]` item in module `ambiguous_transition_include`.
       Fix: keep the machine name unique within the current crate for include-generated transition fragments, or move the transition impl next to its machine definition.
       Loaded `#[machine]` candidates: `FlowMachine` in `invalid_transition_include_ambiguous_machine_name::beta` ($DIR/tests/ui/invalid_transition_include_ambiguous_machine_name.rs:23), `FlowMachine` in `invalid_transition_include_ambiguous_machine_name::alpha` ($DIR/tests/ui/invalid_transition_include_ambiguous_machine_name.rs:36).
 --> tests/ui/support/ambiguous_transition_include.rs
  |
  | impl FlowMachine<Start> {
  |      ^^^^^^^^^^^
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

- Fix: keep the machine name unique within the current crate for include-generated transition fragments, or move the transition impl next to its machine definition.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
