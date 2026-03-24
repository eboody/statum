#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport, StateDescriptor, StateMarker,
    TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state, transition};

#[state]
enum State {
    A,
    B,
}

#[machine]
struct Machine<State> {}

#[transition]
impl Machine<A> {
    fn to_b_or_ghost(self) -> ::core::result::Result<Machine<B>, Machine<Ghost>> {
        if true {
            Ok(self.to_b())
        } else {
            Err(self.to_ghost())
        }
    }

    fn to_b(self) -> Machine<B> {
        self.transition()
    }

    fn to_ghost(self) -> Machine<Ghost> {
        self.transition()
    }
}

fn main() {}
