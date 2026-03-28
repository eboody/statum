#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, LinkedJourneyDescriptor,
    LinkedJourneyStepDescriptor, MachineDescriptor, MachineGraph, MachineIntrospection,
    MachineReference, MachineReferenceTarget, MachineStateIdentity, StateDescriptor, StateMarker,
    TransitionDescriptor, TransitionInventory, UnitState,
};

use statum_macros::{journeys, machine, machine_ref, state};

#[state]
enum TaskState {
    Running,
}

#[machine]
struct Task<TaskState> {}

#[machine_ref(crate::Task<crate::Running>)]
struct TaskReceipt(u64);

#[machine_ref(crate::Task<crate::Running>)]
struct TaskKey(u64);

journeys! {
    journey broken {
        entry: machine!(crate::Task);
        steps: [
            bridge!(crate::TaskReceipt),
            bridge!(crate::TaskKey)
        ];
        outcome: state!(crate::Task, Running);
    }
}

fn main() {}
