#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};
pub use statum_macros::__statum_emit_validator_methods_impl;

use statum_macros::{machine, state};

mod alpha {
    use super::*;
    use statum_macros::validators;

    #[state]
    enum TaskState {
        Draft,
        Done,
    }

    #[machine]
    struct TaskMachine<TaskState> {}

    include!("support/invalid_validators_include_impl_item.rs");
}
