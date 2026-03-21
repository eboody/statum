#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, StateDescriptor, StateMarker,
    TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state};


mod demo {
    use super::*;

    #[state]
    pub enum LightState {
        Off,
    }

    #[machine]
    pub struct LightSwitch<LightState> {
        secret: u8,
        pub visible: u8,
    }
}

fn main() {
    let light = demo::LightSwitch::<demo::Off>::builder()
        .secret(7)
        .visible(9)
        .build();

    let _ = light.visible;
    let _ = light.secret;
}
