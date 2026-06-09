# Validators Default Reports Disabled

Status: tracked compiler-fallback placeholder

Source fixture: `../../statum-macros/tests/ui/invalid_validators_default_reports_disabled.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_validators_default_reports_disabled.stderr`

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

#[state]
pub enum TaskState {
    Draft,
    Done,
}

#[machine]
pub struct TaskMachine<TaskState> {
    name: String,
}

pub struct Row {
    status: &'static str,
}

#[validators(TaskMachine)]
impl Row {
    fn is_draft(&self) -> Result<(), statum_core::Error> {
        let _ = &name;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    fn is_done(&self) -> Result<(), statum_core::Error> {
        let _ = &name;
        if self.status == "done" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}

fn main() {
    let row = Row { status: "draft" };
    let _ = TaskMachine::rebuild(&row)
        .name("todo".to_owned())
        .build_report();
}
```

## Compiler Output

```text
error[E0599]: no method named `build_report` found for struct `__StatumTaskMachineIntoMachine<'__statum_row, __STATUM_SLOT_0_SET>` in the current scope
  --> tests/ui/invalid_validators_default_reports_disabled.rs:53:10
   |
28 |   #[validators(TaskMachine)]
   |   -------------------------- method `build_report` not found for this struct
...
51 |       let _ = TaskMachine::rebuild(&row)
   |  _____________-
52 | |         .name("todo".to_owned())
53 | |         .build_report();
   | |         -^^^^^^^^^^^^ method not found in `__StatumTaskMachineIntoMachine<'_, true>`
   | |_________|
   |
```

## Corrected Example

```toml
# Cargo.toml
statum = { version = "...", features = ["rebuild-reports"] }
```

```rust
let report = TaskMachine::rebuild(&row)
    .name("todo".to_owned())
    .build_report();
```

Without `rebuild-reports`, use the feature-free result builder:

```rust
let machine = TaskMachine::rebuild(&row)
    .name("todo".to_owned())
    .build();
```
