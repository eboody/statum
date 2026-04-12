#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    Branch, CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error,
    MachineDescriptor, MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt,
    RebuildReport, StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

pub mod auth;

fn main() {
    auth::get_user_flow::assert_flow();
}
