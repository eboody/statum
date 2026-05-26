#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state, transition};

#[state]
enum FlowState {
    Draft,
    Review,
}

#[machine]
struct Flow<FlowState> {}

#[transition]
impl Flow<Draft> {
    fn submit(self) -> Flow<Review> {
        self.transition()
    }
}

fn main() {
    let _graph = flow::GRAPH;
    let _state = flow::StateId::Draft;
    let _transition = Flow::<Draft>::SUBMIT;
}
