#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState,
};

// Legacy compatibility import removed.
use statum_macros::{machine, state, validators};
// Builder methods are inherent.

mod support {
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct RoomId(pub u64);
}

mod workflow {
    use super::*;
    use crate::support as chat;

    #[state]
    pub enum WorkflowState {
        Draft,
    }

    #[machine]
    pub struct WorkflowMachine<WorkflowState> {
        pub room_id: chat::RoomId,
    }

    pub struct Row {
        pub status: &'static str,
        pub room_id: chat::RoomId,
    }

    #[validators(WorkflowMachine)]
    impl Row {
        fn is_draft(&self) -> Result<(), statum_core::Error> {
            let _ = &room_id;
            if self.status == "draft" {
                Ok(())
            } else {
                Err(statum_core::Error::InvalidState)
            }
        }
    }
}

fn main() {
    use workflow::workflow_machine::IntoMachinesExt as _;

    let fields = workflow::workflow_machine::Fields {
        room_id: support::RoomId(41),
    };
    let _ = fields.room_id.0;

    let shared = vec![workflow::Row {
        status: "draft",
        room_id: support::RoomId(21),
    }]
    .into_machines()
    .room_id(support::RoomId(22))
    .build();
    match shared.into_iter().next().unwrap().unwrap() {
        workflow::workflow_machine::SomeState::Draft(machine) => {
            let _ = machine.room_id.0;
        }
    }

    let by_row = vec![workflow::Row {
        status: "draft",
        room_id: support::RoomId(31),
    }]
    .into_machines_by(|row| workflow::workflow_machine::Fields {
        room_id: row.room_id.clone(),
    })
    .build();
    match by_row.into_iter().next().unwrap().unwrap() {
        workflow::workflow_machine::SomeState::Draft(machine) => {
            let _ = machine.room_id.0;
        }
    }
}
