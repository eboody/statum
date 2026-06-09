# Validators Relative Path

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/strict_invalid_validators_relative_path.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/strict_invalid_validators_relative_path.stderr`

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

mod flows {
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

struct Row {
    status: &'static str,
}

#[validators(flows::WorkflowMachine)]
impl Row {
    fn is_draft(&self) -> Result<(), statum_core::Error> {
        let _ = &client;
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
error: Error: `#[validators(flows::WorkflowMachine)]` is not accepted in strict introspection mode.
       Found: `#[validators(flows::WorkflowMachine)]`
       Expected: `#[validators(crate::strict_invalid_validators_relative_path::flows::WorkflowMachine)]`
       Fix: use a direct machine path rooted at `crate::`, `self::`, or `super::`.
       Reason: relative multi-segment paths like `flows::WorkflowMachine` can name either module paths or imported aliases, and strict mode only accepts locally readable machine bindings.
  --> tests/ui/strict_invalid_validators_relative_path.rs:31:1
   |
31 | #[validators(flows::WorkflowMachine)]
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
- Expected: `#[validators(crate::strict_invalid_validators_relative_path::flows::WorkflowMachine)]`
- Fix: use a direct machine path rooted at `crate::`, `self::`, or `super::`.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
