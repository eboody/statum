# Machine Missing State Derive

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_machine_missing_state_derive.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_machine_missing_state_derive.stderr`

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

use statum_macros::{machine, state};


#[state]
#[derive(Debug)]
enum BuildState {
    Ready,
    Done,
}

#[machine]
#[derive(Debug, Clone)]
struct BuildMachine<BuildState> {
    name: String,
}
```

## Compiler Output

```text
error: Error: machine `BuildMachine` derives `Clone`, but `#[state]` enum `BuildState` does not.
       Found: `#[derive(Clone)] struct BuildMachine<BuildState> { ... }`
       Expected: `#[derive(Clone)] enum BuildState { ... }`
       Fix: add `#[derive(Clone)]` to `BuildState` so the generated state markers and `BuildMachine` stay compatible.
  --> tests/ui/invalid_machine_missing_state_derive.rs:23:8
   |
23 | struct BuildMachine<BuildState> {
   |        ^^^^^^^^^^^^

error[E0601]: `main` function not found in crate `$CRATE`
  --> tests/ui/invalid_machine_missing_state_derive.rs:25:2
   |
25 | }
   |  ^ consider adding a `main` function to `$DIR/tests/ui/invalid_machine_missing_state_derive.rs`
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

- Found: `#[derive(Clone)] struct BuildMachine<BuildState> { ... }`
- Expected: `#[derive(Clone)] enum BuildState { ... }`
- Fix: add `#[derive(Clone)]` to `BuildState` so the generated state markers and `BuildMachine` stay compatible.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
