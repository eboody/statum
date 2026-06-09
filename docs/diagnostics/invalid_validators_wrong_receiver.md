# Validators Wrong Receiver

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_validators_wrong_receiver.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_validators_wrong_receiver.stderr`

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
enum TaskState {
    Draft,
}

#[machine]
struct TaskMachine<TaskState> {
    name: String,
}

struct DbRow {
    status: &'static str,
}

#[validators(TaskMachine)]
impl DbRow {
    fn is_draft(self) -> Result<(), statum_core::Error> {
        let _ = name;
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
error: Error: validator `is_draft` for `impl DbRow` rebuilding `TaskMachine` state `TaskState::Draft` must take `&self`, not `self`.
       Found: `fn is_draft(self)`
       Expected: `fn is_draft(&self) -> Result<(), _>` or `fn is_draft(&self) -> Validation<()>`
       Note: Machine `TaskMachine` injects these fields by bare name inside validator bodies: `name`. Remove explicit parameters and use those bindings directly.
       Fix: change the receiver to `&self`.
  --> tests/ui/invalid_validators_wrong_receiver.rs:30:17
   |
30 |     fn is_draft(self) -> Result<(), statum_core::Error> {
   |                 ^^^^
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

- Found: `fn is_draft(self)`
- Expected: `fn is_draft(&self) -> Result<(), _>` or `fn is_draft(&self) -> Validation<()>`
- Fix: change the receiver to `&self`.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
