#![allow(unused_imports)]
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
enum WorkflowState {
    Draft(DraftData),
}

struct DraftData {
    title: &'static str,
}

#[machine]
struct WorkflowMachine<WorkflowState> {}

fn main() {
    let _ = WorkflowMachine::<Draft>::builder()
        .state_data(DraftData { title: "first" })
        .state_data(DraftData { title: "second" })
        .build();
}
