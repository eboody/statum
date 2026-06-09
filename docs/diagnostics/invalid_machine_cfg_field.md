# Machine Cfg Field

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_machine_cfg_field.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_machine_cfg_field.stderr`

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
    id: u64,
    #[cfg(any())]
    hidden: &'static str,
}
```

## Compiler Output

```text
error: Error: `#[machine]` struct `WorkflowMachine` field `hidden` uses `#[cfg]`, but Statum does not support conditionally compiled machine fields.
       Found: `#[cfg(any())] hidden: &'static str`
       Expected: an unconditional `hidden` field in `WorkflowMachine`
       Fix: move the cfg gate to the whole `#[machine]` item or split cfg-specific field sets into separate machines.
  --> tests/ui/invalid_machine_cfg_field.rs:21:5
   |
21 | /     #[cfg(any())]
22 | |     hidden: &'static str,
   | |________________________^

error[E0601]: `main` function not found in crate `$CRATE`
  --> tests/ui/invalid_machine_cfg_field.rs:23:2
   |
23 | }
   |  ^ consider adding a `main` function to `$DIR/tests/ui/invalid_machine_cfg_field.rs`
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

- Found: `#[cfg(any())] hidden: &'static str`
- Expected: an unconditional `hidden` field in `WorkflowMachine`
- Fix: move the cfg gate to the whole `#[machine]` item or split cfg-specific field sets into separate machines.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
