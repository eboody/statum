# Validators Wrong Return

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_validators_wrong_return.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_validators_wrong_return.stderr`

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
    InProgress(Progress),
}

struct Progress {
    count: u8,
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
    fn is_draft(&self) -> Result<(), statum_core::Error> {
        let _ = name;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    fn is_in_progress(&self) -> Result<(), statum_core::Error> {
        let _ = name;
        if self.status == "progress" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}
```

## Compiler Output

```text
error: Error: validator `is_in_progress` for `impl DbRow` rebuilding `TaskMachine` state `TaskState::InProgress` must return `Result<Progress, _>` or `Validation<Progress>` (or an equivalent supported alias).
       Found: `Result < (), statum_core :: Error >` with payload `()`
       Expected: `Result<Progress, _>` or `Validation<Progress>`
       Fix: change the validator to return `Progress` for `TaskState::InProgress`.
  --> tests/ui/invalid_validators_wrong_return.rs:44:33
   |
44 |     fn is_in_progress(&self) -> Result<(), statum_core::Error> {
   |                                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error[E0601]: `main` function not found in crate `$CRATE`
  --> tests/ui/invalid_validators_wrong_return.rs:52:2
   |
52 | }
   |  ^ consider adding a `main` function to `$DIR/tests/ui/invalid_validators_wrong_return.rs`
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

- Found: `Result < (), statum_core :: Error >` with payload `()`
- Expected: `Result<Progress, _>` or `Validation<Progress>`
- Fix: change the validator to return `Progress` for `TaskState::InProgress`.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
