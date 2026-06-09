#![allow(dead_code)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state};

#[state]
pub enum WorkflowState {
    Draft,
}

#[machine]
pub struct WorkflowMachine<WorkflowState> {
    r#type: String,
    foo_bar: u8,
    foo__bar: u16,
}

fn main() {
    let builder: WorkflowMachineDraftBuilder = WorkflowMachine::<Draft>::builder();
    let machine = builder
        .r#type("draft".to_owned())
        .foo_bar(1)
        .foo__bar(2)
        .build();

    assert_eq!(machine.r#type, "draft");
    assert_eq!(machine.foo_bar, 1);
    assert_eq!(machine.foo__bar, 2);
}
