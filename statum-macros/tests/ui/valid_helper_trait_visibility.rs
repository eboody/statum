#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};

// Legacy compatibility import removed.
use statum_macros::{machine, state, transition, validators};
// Builder methods are inherent.

pub mod public_flow {
    use super::*;

    #[state]
    pub enum PublicState {
        Draft,
        Review(ReviewData),
        Done,
    }

    #[derive(Clone)]
    pub struct ReviewData {
        pub reviewer: String,
    }

    #[machine]
    pub struct PublicMachine<PublicState> {
        pub tenant: String,
    }

    pub struct Row {
        pub status: &'static str,
    }

    #[validators(PublicMachine)]
    impl Row {
        fn is_draft(&self) -> Result<(), statum_core::Error> {
            let _ = tenant;
            if self.status == "draft" {
                Ok(())
            } else {
                Err(statum_core::Error::InvalidState)
            }
        }

        fn is_review(&self) -> Result<ReviewData, statum_core::Error> {
            let _ = tenant;
            if self.status == "review" {
                Ok(ReviewData {
                    reviewer: tenant.clone(),
                })
            } else {
                Err(statum_core::Error::InvalidState)
            }
        }

        fn is_done(&self) -> Result<(), statum_core::Error> {
            let _ = tenant;
            if self.status == "done" {
                Ok(())
            } else {
                Err(statum_core::Error::InvalidState)
            }
        }
    }

    #[transition]
    impl PublicMachine<Draft> {
        pub fn start_review(self) -> PublicMachine<Review> {
            let reviewer = self.tenant.clone();
            self.transition_with(ReviewData {
                reviewer,
            })
        }
    }

    #[transition]
    impl PublicMachine<Review> {
        pub fn finish(self) -> PublicMachine<Done> {
            self.transition()
        }
    }
}

mod crate_flow {
    use super::*;

    #[state]
    pub(crate) enum WorkflowState {
        Review(ReviewData),
        Done,
    }

    #[derive(Clone)]
    pub(crate) struct ReviewData {
        pub reviewer: String,
    }

    #[machine]
    pub(crate) struct WorkflowMachine<WorkflowState> {
        pub owner: String,
    }

    pub(crate) struct Row {
        pub status: &'static str,
    }

    #[validators(WorkflowMachine)]
    impl Row {
        fn is_review(&self) -> Result<ReviewData, statum_core::Error> {
            let _ = owner;
            if self.status == "review" {
                Ok(ReviewData {
                    reviewer: owner.clone(),
                })
            } else {
                Err(statum_core::Error::InvalidState)
            }
        }

        fn is_done(&self) -> Result<(), statum_core::Error> {
            let _ = owner;
            if self.status == "done" {
                Ok(())
            } else {
                Err(statum_core::Error::InvalidState)
            }
        }
    }

    #[transition]
    impl WorkflowMachine<Review> {
        fn finish(self) -> WorkflowMachine<Done> {
            self.transition()
        }
    }
}

fn main() {
    use crate_flow::workflow_machine::IntoMachinesExt as _;
    use public_flow::public_machine::IntoMachinesExt as _;

    let public_machine = public_flow::PublicMachine::<public_flow::Draft>::builder()
        .tenant("acme".to_string())
        .build();
    let reviewed = public_machine.start_review();
    let _finished = reviewed.finish();

    let public_items = vec![public_flow::Row { status: "review" }]
        .into_machines()
        .tenant("acme".to_string())
        .build();
    match public_items.into_iter().next().unwrap().unwrap() {
        public_flow::public_machine::SomeState::Draft(_machine) => panic!("unexpected draft state"),
        public_flow::public_machine::SomeState::Review(machine) => {
            let _ = machine.state_data.reviewer.as_str();
        }
        public_flow::public_machine::SomeState::Done(_machine) => panic!("unexpected done state"),
    }

    let crate_machine = crate_flow::WorkflowMachine::<crate_flow::Review>::builder()
        .owner("acme".to_string())
        .state_data(crate_flow::ReviewData {
            reviewer: "sam".to_string(),
        })
        .build();
    let _ = crate_machine;

    let crate_items = vec![crate_flow::Row { status: "review" }]
        .into_machines()
        .owner("acme".to_string())
        .build();
    match crate_items.into_iter().next().unwrap().unwrap() {
        crate_flow::workflow_machine::SomeState::Review(machine) => {
            let _ = machine.state_data.reviewer.as_str();
        }
        crate_flow::workflow_machine::SomeState::Done(_machine) => panic!("unexpected done state"),
    }
}
