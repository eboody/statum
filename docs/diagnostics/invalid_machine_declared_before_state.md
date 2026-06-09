# Machine Declared Before State

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_machine_declared_before_state.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_machine_declared_before_state.stderr`

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

#[machine]
struct WorkflowMachine<WorkflowState> {}

#[state]
enum WorkflowState {
    Draft,
}

fn main() {}
```

## Compiler Output

```text
error: Error: machine `WorkflowMachine` could not resolve its `#[state]` enum in module `invalid_machine_declared_before_state`.
       Found: `struct WorkflowMachine<WorkflowState> { ... }`
       Expected: `struct WorkflowMachine<ExpectedState> { ... }` where `ExpectedState` is a `#[state]` enum in `invalid_machine_declared_before_state`
       Fix: make the machine's first generic name the local `#[state]` enum and declare that enum before the machine.
       Reason: Expected a `#[state]` enum named `WorkflowState` in module `invalid_machine_declared_before_state`.
       Note: Statum only resolves `#[state]` enums that have already expanded before this `#[machine]` declaration.
       Note: Source scan found `#[state]` enum `WorkflowState` later in this module on line 17. If that item is active for this build, move it above machine `WorkflowMachine` because Statum resolves these relationships in expansion order.
       Note: No plain enum with that expected name was found in that module either.
       Candidates: No same-named `#[state]` enums were found in other modules of this file.
       Candidates: Available `#[state]` enums in that module: `WorkflowState` in `invalid_machine_declared_before_state` (line 17).
  --> tests/ui/invalid_machine_declared_before_state.rs:13:1
   |
13 | #[machine]
   | ^^^^^^^^^^
   |
   = note: this error originates in the attribute macro `machine` (in Nightly builds, run with -Z macro-backtrace for more info)
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

- Found: `struct WorkflowMachine<WorkflowState> { ... }`
- Expected: `struct WorkflowMachine<ExpectedState> { ... }` where `ExpectedState` is a `#[state]` enum in `invalid_machine_declared_before_state`
- Fix: make the machine's first generic name the local `#[state]` enum and declare that enum before the machine.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
