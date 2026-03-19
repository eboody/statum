#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState,
};
// Legacy compatibility import removed.
use statum_macros::{state, validators};
// Builder methods are inherent.

#[state]
enum TaskState {
    Draft,
}

struct TaskMachine<TaskState> {
    name: String,
    marker: core::marker::PhantomData<TaskState>,
}

struct Row {
    status: &'static str,
}

#[validators(TaskMachine)]
impl Row {
    fn is_draft(&self) -> Result<(), statum_core::Error> {
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}

fn main() {}
