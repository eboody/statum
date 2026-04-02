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

use statum_macros::{machine, state, transition, validators};

#[derive(Clone)]
struct RequestedData {
    queried_transaction_id: String,
}

#[state]
enum AuditState {
    Requested(RequestedData),
    Authorized,
    Done,
}

#[machine]
struct Flow<AuditState> {
    transaction_id: String,
}

impl Flow<Requested> {
    fn queried_transaction_id(&self) -> &str {
        &self.state_data.queried_transaction_id
    }
}

#[transition]
impl Flow<Requested> {
    fn authorize(self) -> Flow<Authorized> {
        self.transition()
    }
}

#[transition]
impl Flow<Authorized> {
    fn finish(self) -> Flow<Done> {
        self.transition()
    }
}

struct AuditRow {
    status: &'static str,
}

#[validators(Flow)]
impl AuditRow {
    fn is_requested(&self) -> Result<RequestedData, statum_core::Error> {
        if self.status == "requested" {
            Ok(RequestedData {
                queried_transaction_id: "txn-1".to_owned(),
            })
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    fn is_authorized(&self) -> Result<(), statum_core::Error> {
        if self.status == "authorized" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    fn is_done(&self) -> Result<(), statum_core::Error> {
        if self.status == "done" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}

fn main() {
    let requested = Flow::<Requested>::builder()
        .transaction_id("txn-1".to_owned())
        .state_data(RequestedData {
            queried_transaction_id: "txn-1".to_owned(),
        })
        .build();

    assert_eq!(requested.queried_transaction_id(), "txn-1");

    let authorized = requested.authorize();
    let _done = authorized.finish();

    let rebuilt = AuditRow { status: "requested" }
        .into_machine()
        .transaction_id("txn-1".to_owned())
        .build()
        .unwrap();

    match rebuilt {
        flow::SomeState::Requested(machine) => {
            assert_eq!(machine.queried_transaction_id(), "txn-1");
        }
        flow::SomeState::Authorized(_) | flow::SomeState::Done(_) => unreachable!(),
    }
}
