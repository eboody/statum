#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport, StateDescriptor, StateMarker,
    TransitionDescriptor, UnitState,
};


use statum_macros::{machine, state};


mod support {
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Text;
}

mod workflow {
    use super::*;
    use crate::support::Text;

    #[state]
    pub enum WorkflowState {
        Draft,
    }

    #[machine]
    pub struct WorkflowMachine<WorkflowState> {
        pub title: Text,
    }

    pub fn assert_alias_surface() {
        let direct = WorkflowMachine::<Draft>::builder().title(Text).build();
        let _ = direct.title;
    }
}

fn main() {
    workflow::assert_alias_surface();
}
