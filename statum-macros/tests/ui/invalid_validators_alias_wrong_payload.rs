#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};
pub type Result<T> = core::result::Result<T, Error>;

// Legacy compatibility import removed.
use statum_macros::{machine, state, validators};
// Builder methods are inherent.

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
    fn is_draft(&self) -> statum::Result<()> {
        let _ = name;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_in_progress(&self) -> statum::Result<()> {
        let _ = name;
        if self.status == "progress" {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }
}
