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

use statum_macros::{machine, state, transition};

mod beta {
    use super::*;

    #[state]
    enum FlowState {
        Start,
        Done,
    }

    #[machine]
    struct FlowMachine<FlowState> {}
}

mod alpha {
    use super::*;

    #[state]
    enum FlowState {
        Start,
        Done,
    }

    #[machine]
    struct FlowMachine<FlowState> {}

    include!("support/ambiguous_transition_include.rs");
}

fn main() {}
