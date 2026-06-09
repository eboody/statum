# Rebuild Many Builder Duplicate Field

Status: tracked compiler-fallback placeholder

Source fixture: `../../statum-macros/tests/ui/invalid_rebuild_many_builder_duplicate_field.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_rebuild_many_builder_duplicate_field.stderr`

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

use statum_macros::{machine, state, validators};

#[state]
enum WorkflowState {
    Draft,
}

#[machine]
struct WorkflowMachine<WorkflowState> {
    name: String,
}

struct Row;

#[validators(WorkflowMachine)]
impl Row {
    fn is_draft(&self) -> Result<(), statum_core::Error> {
        Ok(())
    }
}

fn main() {
    use workflow_machine::IntoMachinesExt as _;

    let _ = vec![Row]
        .into_machines()
        .name("first".to_owned())
        .name("second".to_owned())
        .build();
}
```

## Compiler Output

```text
error[E0599]: no method named `name` found for struct `__StatumWorkflowMachineIntoMachines<true>` in the current scope
  --> tests/ui/invalid_rebuild_many_builder_duplicate_field.rs:38:10
   |
25 |   #[validators(WorkflowMachine)]
   |   ------------------------------ method `name` not found for this struct
...
35 |       let _ = vec![Row]
   |  _____________-
36 | |         .into_machines()
37 | |         .name("first".to_owned())
38 | |         .name("second".to_owned())
   | |         -^^^^ method not found in `__StatumWorkflowMachineIntoMachines<true>`
   | |_________|
   |
   |
   = note: the method was found for
           - `__StatumWorkflowMachineIntoMachines`
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
