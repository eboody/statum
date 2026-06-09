# Legacy Machine Builder

Status: tracked compiler-fallback placeholder

Source fixture: `../../statum-macros/tests/ui/invalid_legacy_machine_builder.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_legacy_machine_builder.stderr`

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

struct Row {
    status: &'static str,
}

#[validators(TaskMachine)]
impl Row {
    fn is_draft(&self) -> Result<(), statum_core::Error> {
        let _ = name;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    fn is_done(&self) -> Result<(), statum_core::Error> {
        let _ = name;
        if self.status == "done" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}

fn main() {
    let row = Row { status: "draft" };
    let _ = row.machine_builder().name("todo".to_string()).build();
}
```

## Compiler Output

```text
error[E0599]: no method named `machine_builder` found for struct `Row` in the current scope
  --> tests/ui/invalid_legacy_machine_builder.rs:53:17
   |
26 | struct Row {
   | ---------- method `machine_builder` not found for this struct
...
53 |     let _ = row.machine_builder().name("todo".to_string()).build();
   |                 ^^^^^^^^^^^^^^^ method not found in `Row`
```

## Corrected Example

```rust
// This fixture is tracked as a compiler-regression placeholder.
// Keep the invalid test, and prefer a nearby valid UI fixture for the corrected shape.
```

## Explanation

- This fixture intentionally records a native Rust compiler error that protects a generated surface or removed legacy API.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
