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


use statum_macros::{machine, state, validators};


mod web {
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Router;
}

mod workflow {
    use super::*;
    use crate::web::Router as AppRouter;

    #[state]
    pub enum WorkflowState {
        Draft,
    }

    #[machine]
    pub struct WorkflowMachine<WorkflowState> {
        pub router: AppRouter,
    }

    pub struct Row {
        pub status: &'static str,
    }

    #[validators(WorkflowMachine)]
    impl Row {
        fn is_draft(&self) -> Result<(), statum_core::Error> {
            let _ = &router;
            if self.status == "draft" {
                Ok(())
            } else {
                Err(statum_core::Error::InvalidState)
            }
        }
    }
}

fn main() {
    let direct = workflow::WorkflowMachine::<workflow::Draft>::builder().router(web::Router).build();
    let _ = direct.router;

    let rebuilt = workflow::Row { status: "draft" }.into_machine().router(web::Router).build().unwrap();
    match rebuilt {
        workflow::workflow_machine::SomeState::Draft(machine) => {
            let _ = machine.router;
        }
    }
}
