#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState,
};
pub use bon;
use statum_macros::{machine, state, validators};
use bon::builder as _;

#[state]
pub enum WorkflowState {
    Draft(DraftData),
    Done,
}

#[derive(Debug, PartialEq)]
pub struct DraftData {
    title: &'static str,
}

#[machine]
pub struct WorkflowMachine<WorkflowState> {
    name: String,
}

pub struct Row {
    status: &'static str,
}

#[validators(WorkflowMachine)]
impl Row {
    fn is_draft(&self) -> Result<DraftData, statum_core::Error> {
        let _ = name;
        if self.status == "draft" {
            Ok(DraftData { title: "from row" })
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
    let machine = WorkflowMachine::<Draft>::builder()
        .name("first".to_owned())
        .name("second".to_owned())
        .state_data(DraftData { title: "first" })
        .state_data(DraftData { title: "second" })
        .build();
    assert_eq!(machine.name, "second");
    assert_eq!(machine.state_data.title, "second");

    let rebuilt = Row { status: "draft" }
        .into_machine()
        .name("first".to_owned())
        .name("second".to_owned())
        .build()
        .unwrap();

    match rebuilt {
        workflow_machine::State::Draft(machine) => {
            assert_eq!(machine.name, "second");
            assert_eq!(machine.state_data.title, "from row");
        }
        workflow_machine::State::Done(_) => panic!("expected draft"),
    }
}
