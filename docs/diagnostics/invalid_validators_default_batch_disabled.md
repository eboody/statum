# Validators Default Batch Disabled

Status: tracked compiler-fallback placeholder

Source fixture: `../../statum-macros/tests/ui/invalid_validators_default_batch_disabled.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_validators_default_batch_disabled.stderr`

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
    use task_machine::IntoMachinesExt as _;

    let _ = vec![Row { status: "draft" }]
        .into_machines()
        .name("todo".to_owned())
        .build();

    let _ = TaskMachine::<UninitializedTaskState>::rebuild_many(vec![Row { status: "done" }]);
}
```

## Compiler Output

```text
error[E0599]: no method named `into_machines` found for struct `Vec<Row>` in the current scope
  --> tests/ui/invalid_validators_default_batch_disabled.rs:53:10
   |
52 |       let _ = vec![Row { status: "draft" }]
   |  _____________-
53 | |         .into_machines()
   | |_________-^^^^^^^^^^^^^
   |
   = help: items from traits can only be used if the trait is implemented and in scope
note: `IntoMachinesExt` defines an item `into_machines`, perhaps you need to implement it
  --> tests/ui/invalid_validators_default_batch_disabled.rs:19:1
   |
19 | #[machine]
   | ^^^^^^^^^^
   = note: this error originates in the attribute macro `machine` (in Nightly builds, run with -Z macro-backtrace for more info)
help: there is a method `into_iter` with a similar name
   |
53 -         .into_machines()
53 +         .into_iter()
   |

error[E0599]: no function or associated item named `rebuild_many` found for struct `TaskMachine<TaskState>` in the current scope
  --> tests/ui/invalid_validators_default_batch_disabled.rs:57:52
   |
19 | #[machine]
   | ---------- function or associated item `rebuild_many` not found for this struct
...
57 |     let _ = TaskMachine::<UninitializedTaskState>::rebuild_many(vec![Row { status: "done" }]);
   |                                                    ^^^^^^^^^^^^ function or associated item not found in `TaskMachine`
   |
help: there is an associated function `rebuild` with a similar name
   |
57 -     let _ = TaskMachine::<UninitializedTaskState>::rebuild_many(vec![Row { status: "done" }]);
57 +     let _ = TaskMachine::<UninitializedTaskState>::rebuild(vec![Row { status: "done" }]);
   |
```

## Corrected Example

```toml
# Cargo.toml
statum = { version = "...", features = ["rebuild-batch"] }
```

```rust
use task_machine::IntoMachinesExt as _;

let machines = vec![Row { status: "draft" }]
    .into_machines()
    .name("todo".to_owned())
    .build();
```

Without `rebuild-batch`, rebuild one row at a time through the default surface:

```rust
let machine = TaskMachine::rebuild(&Row { status: "draft" })
    .name("todo".to_owned())
    .build();
```
