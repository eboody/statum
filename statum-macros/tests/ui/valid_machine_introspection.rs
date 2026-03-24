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
    Draft,
    Review,
    Accepted,
    Rejected,
    Published,
}

#[machine]
struct Flow<FlowState> {}

#[transition]
impl Flow<Draft> {
    fn submit(self) -> Flow<Review> {
        self.transition()
    }
}

#[transition]
impl Flow<Review> {
    fn maybe_decide(self) -> Option<Result<Flow<Accepted>, Flow<Rejected>>> {
        if true {
            Some(Ok(self.accept()))
        } else {
            Some(Err(self.reject()))
        }
    }

    fn accept(self) -> Flow<Accepted> {
        self.transition()
    }

    fn reject(self) -> Flow<Rejected> {
        self.transition()
    }
}

#[transition]
impl Flow<Accepted> {
    fn explain(self) -> Flow<Published> {
        self.transition()
    }
}

#[transition]
impl Flow<Rejected> {
    fn explain(self) -> Flow<Draft> {
        self.transition()
    }
}

fn main() {
    let graph = <Flow<Review> as statum::MachineIntrospection>::GRAPH;
    let maybe_decide = graph
        .transition_from_method(flow::StateId::Review, "maybe_decide")
        .expect("maybe_decide");

    assert_eq!(
        <Flow<Review> as statum::MachineStateIdentity>::STATE_ID,
        flow::StateId::Review
    );
    assert_eq!(
        maybe_decide.id,
        Flow::<Review>::MAYBE_DECIDE
    );
    assert_eq!(
        graph.legal_targets(maybe_decide.id).unwrap(),
        &[flow::StateId::Accepted, flow::StateId::Rejected]
    );
    assert_eq!(graph.transitions_named("explain").count(), 2);
}
