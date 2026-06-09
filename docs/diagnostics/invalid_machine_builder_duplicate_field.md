# Machine Builder Duplicate Field

Status: tracked compiler-fallback placeholder

Source fixture: `../../statum-macros/tests/ui/invalid_machine_builder_duplicate_field.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_machine_builder_duplicate_field.stderr`

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

use statum_macros::{machine, state};

#[state]
enum WorkflowState {
    Draft,
}

#[machine]
struct WorkflowMachine<WorkflowState> {
    name: String,
}

fn main() {
    let _ = WorkflowMachine::<Draft>::builder()
        .name("first".to_owned())
        .name("second".to_owned())
        .build();
}
```

## Compiler Output

```text
error[E0599]: no method named `name` found for struct `WorkflowMachineDraftBuilder<__StatumWorkflowMachineDraftBuilderSetSlot0Name>` in the current scope
  --> tests/ui/invalid_machine_builder_duplicate_field.rs:26:10
   |
18 |   #[machine]
   |   ---------- method `name` not found for this struct
...
24 |       let _ = WorkflowMachine::<Draft>::builder()
   |  _____________-
25 | |         .name("first".to_owned())
26 | |         .name("second".to_owned())
   | |         -^^^^ method not found in `WorkflowMachineDraftBuilder<__StatumWorkflowMachineDraftBuilderSetSlot0Name>`
   | |_________|
   |
   |
   = note: the method was found for
           - `WorkflowMachineDraftBuilder`
```

## Corrected Example

```rust
use statum::{machine, state};

#[state]
enum WorkflowState {
    Draft(DraftData),
}

struct DraftData {
    name: String,
}

#[machine]
struct WorkflowMachine<WorkflowState> {
    owner: String,
}

let machine = WorkflowMachine::draft_builder()
    .owner("ops".to_string())
    .state_data(DraftData { name: "doc".to_string() })
    .build();
```

## Explanation

- This fixture intentionally records a native Rust compiler error that protects a generated surface or removed legacy API.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
