#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};

// Legacy compatibility import removed.
use statum_macros::{machine, state, transition};
// Builder methods are inherent.

#[state]
pub enum WorkflowState {
    Draft(DraftData),
    Review(ReviewData),
    Published,
}

pub struct DraftData {
    title: String,
}

pub struct ReviewData {
    title: String,
    reviewer: String,
}

#[machine]
pub struct WorkflowMachine<WorkflowState> {
    id: u64,
}

#[transition]
impl WorkflowMachine<Draft> {
    fn start_review(self, reviewer: String) -> WorkflowMachine<Review> {
        self.transition_map(|draft| ReviewData {
            title: draft.title,
            reviewer,
        })
    }
}

#[transition]
impl WorkflowMachine<Review> {
    fn publish(self) -> WorkflowMachine<Published> {
        self.transition()
    }
}

fn start_review<M>(machine: M) -> M::Output
where
    M: statum::CanTransitionMap<Review, CurrentData = DraftData>,
{
    machine.transition_map(|draft| ReviewData {
        title: draft.title,
        reviewer: "sam".to_string(),
    })
}

fn main() {
    let draft = WorkflowMachine::<Draft>::builder()
        .id(1)
        .state_data(DraftData {
            title: "Spec".to_string(),
        })
        .build();
    let review = draft.start_review("ada".to_string());
    let _ = review.state_data.reviewer.as_str();

    let draft = WorkflowMachine::<Draft>::builder()
        .id(2)
        .state_data(DraftData {
            title: "Plan".to_string(),
        })
        .build();
    let review: WorkflowMachine<Review> = start_review(draft);
    let _ = review.publish();
}
