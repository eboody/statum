#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};


use statum_macros::{machine, state, transition};


#[state]
enum WorkflowState {
    Draft,
    Done,
}

#[machine]
struct WorkflowMachine<WorkflowState> {}

#[transition]
impl WorkflowMachine<Draft> {
    fn finish(self) -> WorkflowMachine<Done> {
        self.transition()
    }
}

fn assert_transition_trait<T: WorkflowMachineTransitionTo<Done>>(_machine: T) {}

fn main() {
    let machine = WorkflowMachine::<Draft>::builder().build();
    assert_transition_trait(machine);
}
