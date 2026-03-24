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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FlowStateMeta {
    Draft,
}

#[state]
enum FlowState {
    #[present(label = "Draft", metadata = FlowStateMeta::Draft)]
    Draft,
}

#[machine]
struct Flow<FlowState> {}

fn main() {}
