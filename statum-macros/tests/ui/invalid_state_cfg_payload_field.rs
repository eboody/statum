#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};
use statum_macros::state;

#[state]
enum WorkflowState {
    Review {
        reviewer: &'static str,
        #[cfg_attr(any(), allow(dead_code))]
        priority: u8,
    },
}
