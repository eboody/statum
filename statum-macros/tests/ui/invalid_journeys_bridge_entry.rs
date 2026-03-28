#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, LinkedJourneyDescriptor,
    LinkedJourneyStepDescriptor, MachineDescriptor, MachineGraph, MachineIntrospection,
    MachineStateIdentity, StateDescriptor, StateMarker, TransitionDescriptor, TransitionInventory,
    UnitState,
};

use statum_macros::{journeys, machine, state};

#[state]
enum TaskState {
    Running,
}

#[machine]
struct Task<TaskState> {}

journeys! {
    journey broken {
        entry: bridge!(crate::Task);
        steps: [
            machine!(crate::Task)
        ];
        outcome: machine!(crate::Task);
    }
}

fn main() {}
