#![allow(unused_imports)]
extern crate self as statum;
pub use bon;
pub use statum_core::{CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};

use bon::builder as _;
use statum_macros::{machine, state, validators};

mod crate_visible_machine {
    use super::*;

    #[state]
    #[derive(Clone, Debug)]
    pub(crate) enum WorkflowState {
        Draft,
        Review(ReviewData),
        Done,
    }

    #[derive(Clone, Debug)]
    pub(crate) struct ReviewData {
        reviewer: String,
    }

    #[machine]
    pub(crate) struct WorkflowMachine<WorkflowState> {
        owner: String,
    }

    pub(crate) struct Row {
        status: &'static str,
    }

    #[validators(WorkflowMachine)]
    impl Row {
        fn is_draft(&self) -> Result<(), statum_core::Error> {
            let _ = owner;
            if self.status == "draft" {
                Ok(())
            } else {
                Err(statum_core::Error::InvalidState)
            }
        }

        fn is_review(&self) -> Result<ReviewData, statum_core::Error> {
            let _ = owner;
            if self.status == "review" {
                Ok(ReviewData {
                    reviewer: "sam".to_string(),
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

    pub(crate) fn assert_surface() {
        let _draft_copy = Draft.clone();
        let review = Review(ReviewData {
            reviewer: "copy".to_string(),
        });
        let _review_copy = review.clone();

        let draft_state: workflow_machine::State = Row { status: "draft" }
            .into_machine()
            .owner("acme".to_string())
            .build()
            .unwrap();
        let review_state: workflow_machine::State = Row { status: "review" }
            .into_machine()
            .owner("acme".to_string())
            .build()
            .unwrap();

        match draft_state {
            workflow_machine::State::Draft(machine) => {
                let _ = machine.owner;
            }
            workflow_machine::State::Review(_machine) => panic!("unexpected review state"),
            workflow_machine::State::Done(_machine) => panic!("unexpected done state"),
        }

        match review_state {
            workflow_machine::State::Draft(_machine) => panic!("unexpected draft state"),
            workflow_machine::State::Review(machine) => {
                let _ = machine.state_data.reviewer.as_str();
            }
            workflow_machine::State::Done(_machine) => panic!("unexpected done state"),
        }
    }
}

fn main() {
    crate_visible_machine::assert_surface();
}
