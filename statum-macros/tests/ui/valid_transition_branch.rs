#![allow(unused_imports)]
extern crate self as statum;
pub use statum_macros::__statum_emit_validator_methods_impl;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    Branch, CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error,
    MachineDescriptor, MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt,
    RebuildReport, StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state, transition};

#[state]
enum State {
    Draft,
    Accepted,
    Rejected,
}

#[machine]
struct Machine<State> {}

#[transition]
impl Machine<Draft> {
    fn decide(
        self,
        accept: bool,
    ) -> ::statum::Branch<Machine<Accepted>, Machine<Rejected>> {
        if accept {
            Branch::First(self.transition())
        } else {
            Branch::Second(self.transition())
        }
    }
}

fn main() {
    let machine = Machine::<Draft>::builder().build();
    let _ = machine.decide(true);

    let graph = <Machine<Draft> as MachineIntrospection>::GRAPH;
    let decide = graph
        .transition_from_method(machine::StateId::Draft, "decide")
        .unwrap();
    assert_eq!(
        graph.legal_targets(decide.id).unwrap(),
        &[machine::StateId::Accepted, machine::StateId::Rejected]
    );
}
