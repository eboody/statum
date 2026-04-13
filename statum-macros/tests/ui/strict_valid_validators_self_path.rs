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

mod flows {
    use super::*;

    #[state]
    pub enum WorkflowState {
        Draft,
    }

    #[machine]
    pub struct WorkflowMachine<WorkflowState> {
        pub client: String,
    }
}

struct Row {
    status: &'static str,
}

#[validators(self::flows::WorkflowMachine)]
impl Row {
    fn is_draft(&self) -> Result<(), statum_core::Error> {
        let _ = &client;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}

fn main() {
    let rebuilt = self::flows::WorkflowMachine::rebuild(&Row { status: "draft" })
        .client("acme".to_owned())
        .build()
        .unwrap();

    match rebuilt {
        self::flows::workflow_machine::SomeState::Draft(machine) => {
            assert_eq!(machine.client, "acme");
        }
    }
}
