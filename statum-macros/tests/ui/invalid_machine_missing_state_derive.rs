#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, StateDescriptor, StateMarker,
    TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state};


#[state]
#[derive(Debug)]
enum BuildState {
    Ready,
    Done,
}

#[machine]
#[derive(Debug, Clone)]
struct BuildMachine<BuildState> {
    name: String,
}