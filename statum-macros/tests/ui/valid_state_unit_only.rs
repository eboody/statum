#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};
// Legacy compatibility import removed.
use statum_macros::{machine, state};
// Builder methods are inherent.

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