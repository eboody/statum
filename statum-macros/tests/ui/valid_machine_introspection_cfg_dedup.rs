#![allow(unused_imports)]
extern crate self as statum;
pub use statum_macros::__statum_emit_validator_methods_impl;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport, StateDescriptor, StateMarker,
    TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state, transition};

#[state]
enum FlowState {
    Draft,
    Accepted,
    Rejected,
}

#[machine]
struct Flow<FlowState> {}

#[transition]
impl Flow<Draft> {
    #[cfg(any())]
    fn validate(self) -> Flow<Accepted> {
        self.transition()
    }

    #[cfg(not(any()))]
    fn validate(self) -> Flow<Rejected> {
        self.transition()
    }
}

fn main() {
    let graph = <Flow<Draft> as statum::MachineIntrospection>::GRAPH;
    let _validate = graph
        .transition_from_method(flow::StateId::Draft, "validate")
        .expect("validate");
}
