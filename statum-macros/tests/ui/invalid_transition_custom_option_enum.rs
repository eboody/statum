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

enum Option<T> {
    Some(T),
    None,
    Pending,
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
    fn maybe_accept(self, accept: bool) -> Option<Machine<Accepted>> {
        if accept {
            Option::Some(self.accept())
        } else {
            Option::Pending
        }
    }

    fn accept(self) -> Machine<Accepted> {
        self.transition()
    }
}

fn main() {}
