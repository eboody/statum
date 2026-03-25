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

type Maybe<T> = ::core::option::Option<T>;

#[state]
enum State {
    Draft,
    Accepted,
}

#[machine]
struct Machine<State> {}

#[transition]
impl Machine<Draft> {
    fn maybe_accept(self) -> Maybe<Machine<Accepted>> {
        Some(self.transition())
    }
}

fn main() {}
