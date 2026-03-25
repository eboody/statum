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


use statum_macros::state;

#[state]
enum TaskState {
    Draft,
    Review(String),
}

fn assert_state_variant<T: StateVariant>() {}

fn assert_requires_state_data<T: RequiresStateData>() {}

fn assert_does_not_require_state_data<T: DoesNotRequireStateData>() {}

fn main() {
    assert_state_variant::<Draft>();
    assert_requires_state_data::<Review>();
    assert_does_not_require_state_data::<Draft>();
}
