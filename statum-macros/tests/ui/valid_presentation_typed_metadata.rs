#![allow(unused_imports)]
extern crate self as statum;
pub use statum_macros::__statum_emit_validator_methods_impl;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachinePresentation, MachineStateIdentity,
    MachineTransitionRecorder, RebuildAttempt, RebuildReport, RecordedTransition,
    StateDescriptor, StateMarker, TransitionDescriptor, TransitionPresentationInventory,
    UnitState,
};

use statum_macros::{machine, state, transition};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MachineMeta {
    ReviewFlow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StateMeta {
    Queued,
    Reviewing,
    Done,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransitionMeta {
    StartReview,
    Complete,
}

#[state]
enum FlowState {
    #[present(label = "Queued", metadata = StateMeta::Queued)]
    Queued,
    #[present(label = "Reviewing", metadata = StateMeta::Reviewing)]
    Reviewing,
    #[present(label = "Done", metadata = StateMeta::Done)]
    Done,
}

#[machine]
#[presentation_types(
    machine = MachineMeta,
    state = StateMeta,
    transition = TransitionMeta,
)]
#[present(label = "Review Flow", metadata = MachineMeta::ReviewFlow)]
struct Flow<FlowState> {}

#[transition]
impl Flow<Queued> {
    #[present(label = "Start Review", metadata = TransitionMeta::StartReview)]
    fn start_review(self) -> Flow<Reviewing> {
        self.transition()
    }
}

#[transition]
impl Flow<Reviewing> {
    #[present(label = "Complete", metadata = TransitionMeta::Complete)]
    fn complete(self) -> Flow<Done> {
        self.transition()
    }
}

fn main() {
    let presentation = &flow::PRESENTATION;
    assert_eq!(presentation.machine.unwrap().metadata, MachineMeta::ReviewFlow);
    assert_eq!(
        presentation.state(flow::StateId::Queued).unwrap().metadata,
        StateMeta::Queued
    );
    assert_eq!(
        presentation
            .transition(Flow::<Queued>::START_REVIEW)
            .unwrap()
            .metadata,
        TransitionMeta::StartReview
    );

    let event = Flow::<Queued>::try_record_transition_to::<Flow<Reviewing>>(
        Flow::<Queued>::START_REVIEW,
    )
    .unwrap();
    assert_eq!(
        presentation.transition(event.transition).unwrap().metadata,
        TransitionMeta::StartReview
    );
    assert_eq!(
        event.transition_in(<Flow<Queued> as MachineIntrospection>::GRAPH)
            .unwrap()
            .method_name,
        "start_review"
    );
}
