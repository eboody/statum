#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState,
};

pub use bon;
use statum_macros::{machine, state, validators};
use bon::builder as _;

mod support {
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Text;
}

mod workflow {
    use super::*;
    use crate::support::Text;

    #[state]
    pub enum WorkflowState {
        Draft,
    }

    #[machine]
    pub struct WorkflowMachine<WorkflowState> {
        pub title: Text,
    }

    pub struct Row {
        pub status: &'static str,
    }

    #[validators(WorkflowMachine)]
    impl Row {
        fn is_draft(&self) -> Result<(), statum_core::Error> {
            let _ = &title;
            if self.status == "draft" {
                Ok(())
            } else {
                Err(statum_core::Error::InvalidState)
            }
        }
    }
}

fn main() {
    let direct = workflow::WorkflowMachine::<workflow::Draft>::builder()
        .title(support::Text)
        .build();
    let _ = direct.title;

    let rebuilt = workflow::Row { status: "draft" }
        .into_machine()
        .title(support::Text)
        .build()
        .unwrap();
    match rebuilt {
        workflow::workflow_machine::State::Draft(machine) => {
            let _ = machine.title;
        }
    }
}
