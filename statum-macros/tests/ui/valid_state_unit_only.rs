#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, StateDescriptor, StateMarker,
    TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state};


#[state]
pub enum LightState {
    Off,
    On,
}

#[machine]
pub struct Light<LightState> {
    name: String,
}

fn main() {
    let light: Light<Off> = Light::<Off>::builder().name("desk".to_string()).build();
    let _ = light.name;
}