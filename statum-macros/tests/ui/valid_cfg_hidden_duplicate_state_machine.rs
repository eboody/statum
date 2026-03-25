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

use statum_macros::{machine, state, transition, validators};

#[cfg(any())]
#[state]
enum WorkflowState {
    Hidden,
}

#[state]
enum WorkflowState {
    Draft,
    Done,
}

#[cfg(any())]
#[machine]
struct WorkflowMachine<WorkflowState> {
    hidden: u8,
}

#[machine]
struct WorkflowMachine<WorkflowState> {
    name: &'static str,
}

#[transition]
impl WorkflowMachine<Draft> {
    fn finish(self) -> WorkflowMachine<Done> {
        self.transition()
    }
}

struct Row {
    status: &'static str,
}

#[validators(WorkflowMachine)]
impl Row {
    fn is_draft(&self) -> Result<(), statum_core::Error> {
        let _ = &name;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    fn is_done(&self) -> Result<(), statum_core::Error> {
        let _ = &name;
        if self.status == "done" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}

fn main() {
    let machine = WorkflowMachine::<Draft>::builder().name("todo").build();
    let _finished = machine.finish();

    let rebuilt = Row { status: "done" }
        .into_machine()
        .name("todo")
        .build()
        .unwrap();
    match rebuilt {
        workflow_machine::SomeState::Draft(_) => panic!("expected done"),
        workflow_machine::SomeState::Done(machine) => assert_eq!(machine.name, "todo"),
    }
}
