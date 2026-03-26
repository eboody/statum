#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};
pub use statum_macros::__statum_emit_validator_methods_impl;

use statum_macros::{machine, state, validators};

#[state]
enum TaskState {
    Draft,
    Published,
}

#[machine]
struct TaskMachine<TaskState> {}

struct DbRow {
    status: &'static str,
}

#[validators(TaskMachine)]
impl DbRow {
    fn is_draft(&self) -> statum::Result<()> {
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    #[cfg(any())]
    fn is_published(&self) -> statum::Result<()> {
        if self.status == "published" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}
