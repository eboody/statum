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
    Done,
}

#[machine]
struct AlphaMachine<FlowState> {}

#[transition]
impl AlphaMachine<Draft> {
    fn finish(self) -> AlphaMachine<Done> {
        self.transition()
    }
}

#[machine]
struct BetaMachine<FlowState> {}

#[transition]
impl BetaMachine<Draft> {
    fn finish(self) -> BetaMachine<Done> {
        self.transition()
    }
}

fn main() {
    let alpha = AlphaMachine::<Draft>::builder().build();
    let _ = alpha.finish();

    let beta = BetaMachine::<Draft>::builder().build();
    let _ = beta.finish();
}
