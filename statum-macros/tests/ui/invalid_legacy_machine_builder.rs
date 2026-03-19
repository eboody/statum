#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};

// Legacy compatibility import removed.
use statum_macros::{machine, state, validators};
// Builder methods are inherent.

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
