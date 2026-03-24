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
enum FlowState {
    #[present(label = "Draft", description = "Initial work.")]
    Draft,
    Review,
    #[present(label = "Accepted")]
    Accepted,
}

#[machine]
#[present(label = "Review Flow", description = "Small presentation test.")]
struct Flow<FlowState> {}

#[transition]
impl Flow::<Draft> {
    #[present(label = "Submit", description = "Move into review.")]
    fn submit(self) -> Flow<Review> {
        self.transition()
    }
}

#[transition]
impl Flow::<Review> {
    fn approve(self) -> Flow<Accepted> {
        self.transition()
    }
}

fn main() {
    let presentation = &flow::PRESENTATION;
    assert_eq!(
        presentation.machine.unwrap().label,
        Some("Review Flow")
    );
    assert_eq!(
        presentation.machine.unwrap().description,
        Some("Small presentation test.")
    );
    assert_eq!(
        presentation.state(flow::StateId::Draft).unwrap().label,
        Some("Draft")
    );
    assert_eq!(
        presentation
            .state(flow::StateId::Accepted)
            .unwrap()
            .description,
        None
    );
    assert_eq!(
        presentation.transition(Flow::<Draft>::SUBMIT).unwrap().label,
        Some("Submit")
    );
    assert_eq!(
        presentation
            .transition(Flow::<Review>::APPROVE),
        None
    );
}
