# Validators Unknown State Method

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_validators_unknown_state_method.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_validators_unknown_state_method.stderr`

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
    Done,
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

    fn is_archived(&self) -> Result<(), statum_core::Error> {
        let _ = name;
        if self.status == "archived" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}
```

## Compiler Output

```text
error: Error: `#[validators(TaskMachine)]` on `impl DbRow` defines methods that do not match any variant in `TaskState`.
       Found: unknown validator methods: `is_archived`
       Expected: one `is_{state}` method per `TaskMachine` state: `is_draft, is_done`
       Fix: rename or remove methods that do not correspond to a `#[state]` variant.
  --> tests/ui/invalid_validators_unknown_state_method.rs:29:1
   |
29 | #[validators(TaskMachine)]
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = note: this error originates in the attribute macro `validators` (in Nightly builds, run with -Z macro-backtrace for more info)

error[E0601]: `main` function not found in crate `$CRATE`
  --> tests/ui/invalid_validators_unknown_state_method.rs:48:2
   |
48 | }
   |  ^ consider adding a `main` function to `$DIR/tests/ui/invalid_validators_unknown_state_method.rs`
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

- Found: unknown validator methods: `is_archived`
- Expected: one `is_{state}` method per `TaskMachine` state: `is_draft, is_done`
- Fix: rename or remove methods that do not correspond to a `#[state]` variant.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
