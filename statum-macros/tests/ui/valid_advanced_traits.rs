#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport, StateDescriptor, StateMarker,
    TransitionDescriptor, UnitState,
};


use statum_macros::{machine, state, transition};


#[state]
pub enum WorkflowState {
    Draft,
    Review(ReviewData),
    Done,
}

#[derive(Clone)]
pub struct ReviewData {
    reviewer: String,
}

#[machine]
pub struct WorkflowMachine<WorkflowState> {
    id: u64,
}

#[transition]
impl WorkflowMachine<Draft> {
    fn start_review(self, reviewer: String) -> WorkflowMachine<Review> {
        self.transition_with(ReviewData {
            reviewer,
        })
    }
}

#[transition]
impl WorkflowMachine<Review> {
    fn finish(self) -> WorkflowMachine<Done> {
        self.transition()
    }
}

fn start_review<M>(machine: M) -> M::Output
where
    M: statum::CanTransitionWith<ReviewData, NextState = Review>,
{
    machine.transition_with_data(ReviewData {
        reviewer: "sam".to_string(),
    })
}

fn finish<M>(machine: M) -> M::Output
where
    M: statum::CanTransitionTo<Done>,
{
    machine.transition_to()
}

fn assert_unit_state<S: statum::UnitState>() {}

fn assert_data_state<S>()
where
    S: statum::DataState + statum::StateMarker<Data = ReviewData>,
{
}

fn main() {
    assert_unit_state::<Draft>();
    assert_unit_state::<Done>();
    assert_unit_state::<UninitializedWorkflowState>();
    assert_data_state::<Review>();

    let draft = WorkflowMachine::<Draft>::builder().id(1).build();
    let review: WorkflowMachine<Review> = start_review(draft);
    let done: WorkflowMachine<Done> = finish(review);

    let _ = done.id;
}
