#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, StateDescriptor, StateMarker,
    TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state};


#[state]
pub enum ToggleState {
    On,
    Off,
}

#[machine]
pub struct Switch<ToggleState>;

fn main() {
    let _: Switch<On> = Switch::<On>::builder().build();
}