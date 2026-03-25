#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state, validators};

macro_rules! define_state {
    () => {
        #[state]
        pub enum IncludedWorkflowState {
            Draft,
            Review(String),
        }
    };
}

define_state!();

#[machine]
pub struct Workflow<IncludedWorkflowState> {
    id: u64,
}

pub struct DbRow {
    status: &'static str,
}

#[validators(Workflow)]
impl DbRow {
    fn is_draft(&self) -> Result<(), statum_core::Error> {
        let _ = id;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    fn is_review(&self) -> Result<String, statum_core::Error> {
        let _ = id;
        if self.status == "review" {
            Ok("queued".to_string())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}

fn main() {
    let row = DbRow { status: "review" };
    let machine = row.into_machine().id(7).build().unwrap();
    match machine {
        workflow::SomeState::Draft(_machine) => {}
        workflow::SomeState::Review(_machine) => {}
    }
}
