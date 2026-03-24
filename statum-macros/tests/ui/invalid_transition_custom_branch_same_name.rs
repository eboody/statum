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

enum Branch<A, B> {
    First(A),
    Second(B),
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
    fn decide(self, accept: bool) -> Branch<Machine<Accepted>, Machine<Rejected>> {
        if accept {
            Branch::First(self.accept())
        } else {
            Branch::Pending
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
