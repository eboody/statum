# Validators Relative Path Alias

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_validators_relative_path_alias.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_validators_relative_path_alias.stderr`

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

mod workflow_defs {
    use super::*;

    #[state]
    pub enum WorkflowState {
        Draft,
    }

    #[machine]
    pub struct WorkflowMachine<WorkflowState> {
        pub client: String,
    }
}

use crate::workflow_defs as flows;

struct Row {
    status: &'static str,
}

#[validators(flows::WorkflowMachine)]
impl Row {
    fn is_draft(&self) -> Result<(), statum_core::Error> {
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}

fn main() {}
```

## Compiler Output

```text
error: Error: `#[validators(flows::WorkflowMachine)]` could not resolve a matching `#[machine]` in module `invalid_validators_relative_path_alias::flows`.
       Found: `#[validators(flows::WorkflowMachine)]`
       Expected: `#[validators(crate::invalid_validators_relative_path_alias::workflow_defs::WorkflowMachine)]`
       Fix: point `#[validators(...)]` at the Statum machine type declared in that module and declare that `#[machine]` item before this validators impl.
       Reason: Statum only resolves `#[machine]` items that have already expanded before this `#[validators]` impl.
       Assumption: Path note: Statum interpreted `flows::WorkflowMachine` as the local child-module path `self::flows::WorkflowMachine`.
                   Imported aliases and re-exports are not supported in `#[validators(...)]` path resolution.
                   If you meant that local module, declare the `#[machine]` there or spell it `#[validators(self::flows::WorkflowMachine)]` for clarity. If you meant a different module, anchor the real path, for example `#[validators(crate::invalid_validators_relative_path_alias::workflow_defs::WorkflowMachine)]`.
       Note: No plain struct with that name was found in this module either.
       Candidates: Same-named `#[machine]` items elsewhere in this file: `WorkflowMachine` in `invalid_validators_relative_path_alias::workflow_defs` (line 22).
       Candidates: No `#[machine]` items were found in this module.
       Help: Correct shape: `#[validators(crate::invalid_validators_relative_path_alias::workflow_defs::WorkflowMachine)] impl PersistedRow { ... }`.
  --> tests/ui/invalid_validators_relative_path_alias.rs:33:1
   |
33 | #[validators(flows::WorkflowMachine)]
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
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

- Found: `#[validators(flows::WorkflowMachine)]`
- Expected: `#[validators(crate::invalid_validators_relative_path_alias::workflow_defs::WorkflowMachine)]`
- Fix: point `#[validators(...)]` at the Statum machine type declared in that module and declare that `#[machine]` item before this validators impl.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
