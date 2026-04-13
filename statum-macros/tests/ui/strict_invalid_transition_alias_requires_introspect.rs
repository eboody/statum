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

type Next =
    ::core::result::Result<WorkflowMachine<Review>, WorkflowMachine<Rejected>>;

#[state]
enum WorkflowState {
    Draft,
    Review,
    Rejected,
}

#[machine]
struct WorkflowMachine<WorkflowState> {}

#[transition]
impl WorkflowMachine<Draft> {
    fn submit(self, approve: bool) -> Next {
        if approve {
            Ok(self.review())
        } else {
            Err(self.reject())
        }
    }

    fn review(self) -> WorkflowMachine<Review> {
        self.transition()
    }

    fn reject(self) -> WorkflowMachine<Rejected> {
        self.transition()
    }
}

fn main() {}
