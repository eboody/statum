#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

#[path = "support/strict_validators_external_layout/flows.rs"]
mod flows;
#[path = "support/strict_validators_external_layout/rebuilders.rs"]
mod rebuilders;

fn main() {
    rebuilders::assert_rebuild();
}
