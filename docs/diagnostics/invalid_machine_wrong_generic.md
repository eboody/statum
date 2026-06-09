# Machine Wrong Generic

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_machine_wrong_generic.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_machine_wrong_generic.stderr`

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
enum MachineState {
    Ready,
}

#[machine]
struct Machine<S: Clone> {
    client: String,
}
```

## Compiler Output

```text
error: Error: machine `Machine` uses `S: Clone` as its state generic, but the `#[state]` enum in this module is `MachineState`.
       Found: `struct Machine<S: Clone> { ... }`
       Expected: `struct Machine<MachineState> { ... }`
       Fix: declare `Machine<MachineState>`.
  --> tests/ui/invalid_machine_wrong_generic.rs:20:16
   |
20 | struct Machine<S: Clone> {
   |                ^^^^^^^^

error[E0601]: `main` function not found in crate `$CRATE`
  --> tests/ui/invalid_machine_wrong_generic.rs:22:2
   |
22 | }
   |  ^ consider adding a `main` function to `$DIR/tests/ui/invalid_machine_wrong_generic.rs`
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

- Found: `struct Machine<S: Clone> { ... }`
- Expected: `struct Machine<MachineState> { ... }`
- Fix: declare `Machine<MachineState>`.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
