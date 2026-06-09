# Legacy Transition Helper Trait

Status: tracked compiler-fallback placeholder

Source fixture: `../../statum-macros/tests/ui/invalid_legacy_transition_helper_trait.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_legacy_transition_helper_trait.stderr`

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
enum WorkflowState {
    Draft,
    Done,
}

#[machine]
struct WorkflowMachine<WorkflowState> {}

#[transition]
impl WorkflowMachine<Draft> {
    fn finish(self) -> WorkflowMachine<Done> {
        self.transition()
    }
}

fn assert_transition_trait<T: WorkflowMachineTransitionTo<Done>>(_machine: T) {}

fn main() {
    let machine = WorkflowMachine::<Draft>::builder().build();
    assert_transition_trait(machine);
}
```

## Compiler Output

```text
error[E0405]: cannot find trait `WorkflowMachineTransitionTo` in this scope
  --> tests/ui/invalid_legacy_transition_helper_trait.rs:31:31
   |
31 | fn assert_transition_trait<T: WorkflowMachineTransitionTo<Done>>(_machine: T) {}
   |                               ^^^^^^^^^^^^^^^^^^^^^^^^^^^ not found in this scope
```

## Corrected Example

```rust
// This fixture is tracked as a compiler-regression placeholder.
// Keep the invalid test, and prefer a nearby valid UI fixture for the corrected shape.
```

## Explanation

- This fixture intentionally records a native Rust compiler error that protects a generated surface or removed legacy API.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
