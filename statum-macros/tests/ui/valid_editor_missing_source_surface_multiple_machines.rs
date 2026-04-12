#![allow(unused_imports)]
extern crate self as statum;
pub use statum_macros::__statum_emit_validator_methods_impl;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state};

mod inbound {
    use super::*;

    #[state]
    pub enum State {
        Accepted,
    }

    #[machine]
    pub struct Flow<State> {
        transaction_id: u64,
    }
}

mod outbound {
    use super::*;

    #[state]
    pub enum State {
        Released,
    }

    #[machine]
    pub struct Flow<State> {
        transaction_id: u64,
    }
}

mod submit_task {
    use super::*;

    #[state]
    pub enum State {
        Accepted,
    }

    #[machine]
    pub struct Flow<State> {
        transaction_id: u64,
    }
}

fn main() {
    let _ = inbound::Flow::<inbound::Accepted>::builder()
        .transaction_id(1)
        .build();
    let _ = outbound::Flow::<outbound::Released>::builder()
        .transaction_id(2)
        .build();
    let _ = submit_task::Flow::<submit_task::Accepted>::builder()
        .transaction_id(3)
        .build();
}
