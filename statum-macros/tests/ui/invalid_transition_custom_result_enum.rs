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

enum Result<T, E> {
    Ok(T),
    Err(E),
    Pending,
}

#[state]
enum State {
    Draft,
    Accepted,
    Rejected,
}

#[machine]
struct Machine<State> {}

#[transition]
impl Machine<Draft> {
    fn decide(self, accept: bool) -> Result<Machine<Accepted>, Machine<Rejected>> {
        if accept {
            Result::Ok(self.accept())
        } else {
            Result::Pending
        }
    }

    fn accept(self) -> Machine<Accepted> {
        self.transition()
    }

    fn reject(self) -> Machine<Rejected> {
        self.transition()
    }
}

fn main() {}
