# Validators Declared Before Machine

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_validators_declared_before_machine.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_validators_declared_before_machine.stderr`

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

use statum_macros::{machine, state, validators};

#[state]
enum WorkflowState {
    Draft,
}

struct Row {
    status: &'static str,
}

#[validators(WorkflowMachine)]
impl Row {
    fn is_draft(&self) -> Result<(), statum_core::Error> {
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}

#[machine]
struct WorkflowMachine<WorkflowState> {}

fn main() {}
```

## Compiler Output

```text
error: Error: `#[validators(WorkflowMachine)]` could not resolve a matching `#[machine]` in module `invalid_validators_declared_before_machine`.
       Found: `#[validators(WorkflowMachine)]`
       Expected: `#[validators(WorkflowMachine)]`
       Fix: point `#[validators(...)]` at the Statum machine type declared in that module and declare that `#[machine]` item before this validators impl.
       Reason: Statum only resolves `#[machine]` items that have already expanded before this `#[validators]` impl.
       Note: Source scan found `#[machine]` item `WorkflowMachine` later in this module on line 34. If that item is active for this build, move it above this `#[validators]` impl because Statum resolves these relationships in expansion order.
       Note: No plain struct with that name was found in this module either.
       Candidates: No same-named `#[machine]` items were found in other modules of this file.
       Candidates: Available `#[machine]` items in this module: `WorkflowMachine` in `invalid_validators_declared_before_machine` (line 34).
       Help: Correct shape: `#[validators(WorkflowMachine)] impl PersistedRow { ... }`.
  --> tests/ui/invalid_validators_declared_before_machine.rs:22:1
   |
22 | #[validators(WorkflowMachine)]
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: this error originates in the attribute macro `validators` (in Nightly builds, run with -Z macro-backtrace for more info)
```

## Corrected Example

```rust
use statum::{machine, state, validators, Result};

#[state]
enum TaskState {
    Draft,
    InProgress(Progress),
}

struct Progress;

#[machine]
struct TaskMachine<TaskState> {}

struct DbRow;

#[validators(TaskMachine)]
impl DbRow {
    fn is_draft(&self) -> Result<()> {
        Ok(())
    }

    fn is_in_progress(&self) -> Result<Progress> {
        Ok(Progress)
    }
}
```

## Explanation

- Found: `#[validators(WorkflowMachine)]`
- Expected: `#[validators(WorkflowMachine)]`
- Fix: point `#[validators(...)]` at the Statum machine type declared in that module and declare that `#[machine]` item before this validators impl.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
