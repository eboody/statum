#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, StateDescriptor, StateMarker,
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

struct Row {
    status: &'static str,
}

#[validators(DoesNotExist)]
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
