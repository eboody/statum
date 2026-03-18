#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};

pub use bon;
use statum_macros::{machine, state, validators};
use bon::builder as _;

mod workflow {
    use super::*;

    #[state]
    pub enum WorkflowState {
        Draft,
        Published,
    }

    #[machine]
    pub struct WorkflowMachine<WorkflowState> {
        pub tenant: String,
        pub priority: u8,
    }

    pub struct Row {
        pub tenant: String,
        pub priority: u8,
        pub status: &'static str,
    }

    #[validators(WorkflowMachine)]
    impl Row {
        fn is_draft(&self) -> Result<(), statum_core::Error> {
            if self.status == "draft" {
                Ok(())
            } else {
                Err(statum_core::Error::InvalidState)
            }
        }

        fn is_published(&self) -> Result<(), statum_core::Error> {
            if self.status == "published" {
                Ok(())
            } else {
                Err(statum_core::Error::InvalidState)
            }
        }
    }

    pub fn local_rebuild(rows: Vec<Row>) -> Vec<Result<workflow_machine::SomeState, statum_core::Error>> {
        rows.into_machines_by(|row| workflow_machine::Fields {
            tenant: row.tenant.clone(),
            priority: row.priority,
        })
        .build()
    }
}

fn main() {
    use workflow::workflow_machine::IntoMachinesExt as _;

    let local = workflow::local_rebuild(vec![workflow::Row {
        tenant: "acme".to_string(),
        priority: 1,
        status: "draft",
    }]);
    match local.into_iter().next().unwrap().unwrap() {
        workflow::workflow_machine::SomeState::Draft(machine) => {
            let _ = (machine.tenant, machine.priority);
        }
        workflow::workflow_machine::SomeState::Published(_) => panic!("unexpected state"),
    }

    let remote = vec![workflow::Row {
        tenant: "globex".to_string(),
        priority: 2,
        status: "published",
    }]
    .into_machines_by(|row| workflow::workflow_machine::Fields {
        tenant: row.tenant.clone(),
        priority: row.priority,
    })
    .build();

    match remote.into_iter().next().unwrap().unwrap() {
        workflow::workflow_machine::SomeState::Draft(_) => panic!("unexpected state"),
        workflow::workflow_machine::SomeState::Published(machine) => {
            let _ = (machine.tenant, machine.priority);
        }
    }
}
