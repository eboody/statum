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

enum Decision<A, B> {
    Accept(A),
    Reject(B),
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
    fn decide(self, accept: bool) -> Decision<Machine<Accepted>, Machine<Rejected>> {
        if accept {
            Decision::Accept(self.transition())
        } else {
            Decision::Reject(self.transition())
        }
    }
}

fn main() {}
