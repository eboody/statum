#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, StateDescriptor, StateMarker,
    TransitionDescriptor, UnitState,
};


use statum_macros::{machine, state, transition};


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
pub struct WorkflowMachine<WorkflowState> {}

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

fn main() {
    let review = WorkflowMachine::<Review>::builder()
        .state_data(ReviewData {
            title: "Spec".to_string(),
            reviewer: "ada".to_string(),
        })
        .build();

    let _ = review.transition_map(|data| data);
}
