#![allow(unused_imports)]
extern crate self as statum;
pub use statum_macros::__statum_emit_validator_methods_impl;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    Attested, CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error,
    MachineDescriptor, MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt,
    RebuildReport, StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state, transition};

#[state]
enum PaymentState {
    Authorized,
    Captured,
}

#[machine]
struct Payment<PaymentState> {}

#[transition]
impl Payment<Authorized> {
    fn capture(self) -> Payment<Captured> {
        self.transition()
    }
}

#[state]
enum WorkflowState {
    Ready,
    Done,
}

#[machine]
struct Workflow<WorkflowState> {}

#[transition]
impl Workflow<Ready> {
    fn finish(
        self,
        #[via(crate::payment::via::Capture)]
        left: Payment<Captured>,
        #[via(crate::payment::via::Capture)]
        right: Payment<Captured>,
    ) -> Workflow<Done> {
        let _ = (left, right);
        self.transition()
    }
}

fn main() {}
