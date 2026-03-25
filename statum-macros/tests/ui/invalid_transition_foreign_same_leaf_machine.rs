#![allow(unused_imports)]
extern crate self as statum;
pub use statum_macros::__statum_emit_validator_methods_impl;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state, transition};

mod other {
    pub struct Machine<State>(pub core::marker::PhantomData<State>);
}

#[state]
enum State {
    Draft,
    Accepted,
}

#[machine]
struct Machine<State> {}

#[transition]
impl Machine<Draft> {
    fn finish(self) -> other::Machine<Accepted> {
        other::Machine(core::marker::PhantomData)
    }
}

fn main() {}
