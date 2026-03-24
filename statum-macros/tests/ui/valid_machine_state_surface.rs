#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport, StateDescriptor, StateMarker,
    TransitionDescriptor, UnitState,
};


use statum_macros::{machine, state, validators};


mod private_machine {
    use super::*;

    #[state]
    enum WorkflowState {
        Draft,
        Done,
    }

    #[machine]
    struct WorkflowMachine<WorkflowState> {
        id: u64,
    }

    pub fn assert_private_surface() {
        let machine = WorkflowMachine::<Draft>::builder().id(1).build();
        let state = workflow_machine::SomeState::Draft(machine);

        match state {
            workflow_machine::SomeState::Draft(machine) => {
                let _ = machine.id;
            }
            workflow_machine::SomeState::Done(_machine) => {}
        }
    }
}

#[state]
pub enum TaskState {
    Draft,
    Done,
}

#[machine]
pub struct TaskMachine<TaskState> {
    name: String,
}

pub struct Row {
    status: &'static str,
}

#[validators(TaskMachine)]
impl Row {
    fn is_draft(&self) -> Result<(), statum_core::Error> {
        let _ = name;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    fn is_done(&self) -> Result<(), statum_core::Error> {
        let _ = name;
        if self.status == "done" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}

fn main() {
    private_machine::assert_private_surface();

    let row = Row { status: "draft" };
    let state: task_machine::SomeState = row
        .into_machine()
        .name("todo".to_string())
        .build()
        .unwrap();

    match state {
        task_machine::SomeState::Draft(machine) => {
            let _ = machine.name;
        }
        task_machine::SomeState::Done(_machine) => {}
    }

    let alias_state: task_machine::State =
        task_machine::SomeState::Draft(TaskMachine::<Draft>::builder().name("alias".to_string()).build());

    match alias_state {
        task_machine::State::Draft(machine) => {
            let _ = machine.name;
        }
        task_machine::State::Done(_machine) => {}
    }
}
