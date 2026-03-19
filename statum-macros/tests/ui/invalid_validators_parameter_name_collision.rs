#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState,
};
// Legacy compatibility import removed.
use statum_macros::{machine, state, validators};
// Builder methods are inherent.

#[state]
enum TaskState {
    Draft,
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
    fn is_draft(&self, name: &str) -> Result<(), statum_core::Error> {
        if self.status == name {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}

fn main() {}
