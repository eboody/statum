#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport, StateDescriptor, StateMarker,
    TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state, validators};

#[state]
pub enum ReviewState {
    Draft {
        reviewer: String,
        priority: u8,
    },
    Published,
}

#[machine]
pub struct Document<ReviewState> {
    id: u64,
}

pub struct Row {
    reviewer: &'static str,
}

#[validators(Document)]
impl Row {
    fn is_draft(&self) -> Result<DraftData, statum_core::Error> {
        let _ = id;
        Ok(DraftData {
            reviewer: self.reviewer.to_owned(),
            priority: 3,
        })
    }

    fn is_published(&self) -> Result<(), statum_core::Error> {
        Err(statum_core::Error::InvalidState)
    }
}

fn main() {
    let machine = Document::<Draft>::builder()
        .id(1)
        .state_data(DraftData {
            reviewer: "sam".to_owned(),
            priority: 2,
        })
        .build();
    assert_eq!(machine.state_data.reviewer, "sam");
    assert_eq!(machine.state_data.priority, 2);

    let rebuilt = Row { reviewer: "alex" }
        .into_machine()
        .id(7)
        .build()
        .unwrap();
    match rebuilt {
        document::SomeState::Draft(machine) => {
            assert_eq!(machine.state_data.reviewer, "alex");
            assert_eq!(machine.state_data.priority, 3);
        }
        _ => panic!("expected draft"),
    }
}
