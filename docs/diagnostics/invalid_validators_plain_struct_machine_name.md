# Validators Plain Struct Machine Name

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_validators_plain_struct_machine_name.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_validators_plain_struct_machine_name.stderr`

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

use statum_macros::{state, validators};


#[state]
enum TaskState {
    Draft,
}

struct TaskMachine<TaskState> {
    name: String,
    marker: core::marker::PhantomData<TaskState>,
}

struct Row {
    status: &'static str,
}

#[validators(TaskMachine)]
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
error: Error: `#[validators(TaskMachine)]` could not resolve a matching `#[machine]` in module `invalid_validators_plain_struct_machine_name`.
       Found: `#[validators(TaskMachine)]`
       Expected: `#[validators(TaskMachine)]`
       Fix: point `#[validators(...)]` at the Statum machine type declared in that module and declare that `#[machine]` item before this validators impl.
       Reason: Statum only resolves `#[machine]` items that have already expanded before this `#[validators]` impl.
       Note: A struct named `TaskMachine` exists on line 19, but it is not annotated with `#[machine]`.
       Candidates: No same-named `#[machine]` items were found in other modules of this file.
       Candidates: No `#[machine]` items were found in this module.
       Help: Correct shape: `#[validators(TaskMachine)] impl PersistedRow { ... }`.
  --> tests/ui/invalid_validators_plain_struct_machine_name.rs:28:1
   |
28 | #[validators(TaskMachine)]
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^
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

- Found: `#[validators(TaskMachine)]`
- Expected: `#[validators(TaskMachine)]`
- Fix: point `#[validators(...)]` at the Statum machine type declared in that module and declare that `#[machine]` item before this validators impl.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
