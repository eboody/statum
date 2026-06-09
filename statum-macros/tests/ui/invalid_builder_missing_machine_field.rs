#![allow(dead_code)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state};

#[state]
pub enum WorkflowState {
    Draft(DraftData),
}

pub struct DraftData {
    title: &'static str,
}

#[machine]
pub struct WorkflowMachine<WorkflowState> {
    name: String,
}

fn main() {
    let _ = WorkflowMachine::<Draft>::builder()
        .state_data(DraftData { title: "draft" })
        .build();
}
