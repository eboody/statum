#![allow(unused_imports)]
extern crate self as statum;
pub use statum_macros::__statum_emit_validator_methods_impl;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    linked_journeys, CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error,
    LinkedJourneyDescriptor, LinkedJourneyStepDescriptor, MachineDescriptor, MachineGraph,
    MachineIntrospection, MachineReference, MachineReferenceTarget, MachineStateIdentity,
    RebuildAttempt, RebuildReport, StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::{journeys, machine, machine_ref, state, transition, validators};

#[state]
enum TaskState {
    Running,
}

#[machine]
struct Task<TaskState> {}

#[state]
enum WorkflowState {
    Draft,
    InProgress(Task<Running>),
    Done,
}

#[machine]
struct Workflow<WorkflowState> {}

struct WorkflowRow {
    status: &'static str,
}

#[validators(Workflow)]
impl WorkflowRow {
    fn is_draft(&self) -> Result<(), statum_core::Error> {
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    fn is_in_progress(&self) -> Result<Task<Running>, statum_core::Error> {
        if self.status == "running" {
            Ok(Task::<Running>::builder().build())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    fn is_done(&self) -> Result<(), statum_core::Error> {
        if self.status == "done" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}

#[machine_ref(crate::Task<crate::Running>)]
struct TaskReceipt(u64);

journeys! {
    journey workflow_story {
        label: "Workflow Story";
        docs: "Tracks workflow reconstruction and task handoff.";
        entry: validator!(crate::WorkflowRow => crate::Workflow);
        steps: [
            state!(crate::Workflow, InProgress),
            bridge!(crate::TaskReceipt),
            machine!(crate::Task)
        ];
        outcome: state!(crate::Task, Running);
    }
}

fn main() {
    let journeys = linked_journeys();
    assert_eq!(journeys.len(), 1);
    assert_eq!(journeys[0].id, "workflow_story");
}
