#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state, transition};

#[cfg(any())]
#[state]
enum HiddenWorkflowState {
    Hidden,
}

#[cfg(any())]
#[machine]
struct HiddenWorkflowMachine<HiddenWorkflowState> {}

#[state]
enum WorkflowState {
    Draft,
    Review,
    Archived,
    Published,
}

#[machine]
struct WorkflowMachine<WorkflowState> {}

#[transition]
impl WorkflowMachine<Draft> {
    #[cfg(any())]
    fn advance(self) -> WorkflowMachine<Archived> {
        self.transition()
    }

    #[cfg(not(any()))]
    fn advance(self) -> WorkflowMachine<Review> {
        self.transition()
    }
}

#[transition]
impl WorkflowMachine<Review> {
    fn publish(self) -> WorkflowMachine<Published> {
        self.transition()
    }
}

fn main() {
    let graph = <WorkflowMachine<Draft> as statum::MachineIntrospection>::GRAPH;

    assert!(graph.state(workflow_machine::StateId::Draft).is_some());
    assert!(graph.state(workflow_machine::StateId::Review).is_some());
    assert!(graph.state(workflow_machine::StateId::Archived).is_some());
    assert!(graph.state(workflow_machine::StateId::Published).is_some());

    let advance = graph
        .transition_from_method(workflow_machine::StateId::Draft, "advance")
        .expect("cfg-pruned active transition should be present");
    assert_eq!(
        graph.legal_targets(advance.id).unwrap(),
        &[workflow_machine::StateId::Review]
    );
    assert!(
        graph
            .transitions_named("advance")
            .all(|transition| transition.to == &[workflow_machine::StateId::Review]),
        "cfg-disabled Archived edge must not leak into metadata"
    );
}
