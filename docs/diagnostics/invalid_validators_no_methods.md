# Validators No Methods

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_validators_no_methods.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_validators_no_methods.stderr`

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
impl DbRow {}
```

## Compiler Output

```text
error: Error: `#[validators(TaskMachine)]` on `impl DbRow` must define at least one validator method.
       Expected: one method per `TaskState` variant: `is_draft`
       Fix: add validator methods like `fn is_draft(&self) -> Result<(), _>`.
  --> tests/ui/invalid_validators_no_methods.rs:28:1
   |
28 | #[validators(TaskMachine)]
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: this error originates in the attribute macro `validators` (in Nightly builds, run with -Z macro-backtrace for more info)

error[E0601]: `main` function not found in crate `$CRATE`
  --> tests/ui/invalid_validators_no_methods.rs:29:14
   |
29 | impl DbRow {}
   |              ^ consider adding a `main` function to `$DIR/tests/ui/invalid_validators_no_methods.rs`
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

- Expected: one method per `TaskState` variant: `is_draft`
- Fix: add validator methods like `fn is_draft(&self) -> Result<(), _>`.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
