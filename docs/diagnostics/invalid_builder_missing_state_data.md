# Builder Missing State Data

Status: tracked compiler-fallback placeholder

Source fixture: `../../statum-macros/tests/ui/invalid_builder_missing_state_data.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_builder_missing_state_data.stderr`

## Broken Example

```rust
#![allow(dead_code)]
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
pub enum WorkflowState {
    Draft(DraftData),
}

pub struct DraftData {
    title: &'static str,
}

#[machine]
pub struct WorkflowMachine<WorkflowState> {
    name: String,
}

fn main() {
    let _ = WorkflowMachine::<Draft>::builder()
        .name("draft".to_owned())
        .build();
}
```

## Compiler Output

```text
error[E0599]: no method named `build` found for struct `WorkflowMachineDraftBuilder<__StatumWorkflowMachineDraftBuilderMissingSlot0StateData, __StatumWorkflowMachineDraftBuilderSetSlot1Name>` in the current scope
  --> tests/ui/invalid_builder_missing_state_data.rs:30:10
   |
22 |   #[machine]
   |   ---------- method `build` not found for this struct
...
28 |       let _ = WorkflowMachine::<Draft>::builder()
   |  _____________-
29 | |         .name("draft".to_owned())
30 | |         .build();
   | |         -^^^^^ method not found in `WorkflowMachineDraftBuilder<__StatumWorkflowMachineDraftBuilderMissingSlot0StateData, __StatumWorkflowMachineDraftBuilderSetSlot1Name>`
   | |_________|
   |
   |
   = note: the method was found for
           - `WorkflowMachineDraftBuilder<__StatumWorkflowMachineDraftBuilderSetSlot0StateData, __StatumWorkflowMachineDraftBuilderSetSlot1Name>`
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
