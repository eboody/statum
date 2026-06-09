# Validators Missing Machine Path

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_validators_missing_machine_path.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_validators_missing_machine_path.stderr`

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

use statum_macros::{validators, machine, state};

#[state]
enum WorkflowState {
    Draft,
}

#[machine]
struct WorkflowMachine<WorkflowState> {}

struct Row;

#[validators]
impl Row {
    fn is_draft(&self) -> Result<(), statum_core::Error> {
        Ok(())
    }
}

fn main() {}
```

## Compiler Output

```text
error: Error: `#[validators(...)]` requires a machine path.
       Expected: `#[validators(WorkflowMachine)] impl PersistedRow { ... }`
       Fix: pass the target Statum machine type in the attribute, for example `#[validators(self::flow::WorkflowMachine)]`.
  --> tests/ui/invalid_validators_missing_machine_path.rs:23:1
   |
23 | #[validators]
   | ^^^^^^^^^^^^^
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

- Expected: `#[validators(WorkflowMachine)] impl PersistedRow { ... }`
- Fix: pass the target Statum machine type in the attribute, for example `#[validators(self::flow::WorkflowMachine)]`.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
