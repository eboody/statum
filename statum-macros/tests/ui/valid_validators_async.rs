#![allow(unused_imports)]
extern crate self as statum;
pub use statum_macros::__statum_emit_validator_methods_impl;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport, StateDescriptor, StateMarker,
    TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state, validators};


#[state]
pub enum JobState {
    Queued,
    Running(JobData),
    Complete,
}

pub struct JobData {
    id: u64,
}

#[machine]
pub struct JobMachine<JobState> {
    worker: String,
}

pub struct JobRow {
    status: &'static str,
}

#[validators(JobMachine)]
impl JobRow {
    async fn is_queued(&self) -> Result<(), statum_core::Error> {
        let _ = worker;
        if self.status == "queued" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    async fn is_running(&self) -> Result<JobData, statum_core::Error> {
        let _ = worker;
        if self.status == "running" {
            Ok(JobData { id: 1 })
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    async fn is_complete(&self) -> Result<(), statum_core::Error> {
        let _ = worker;
        if self.status == "complete" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}

fn main() {
    let row = JobRow { status: "queued" };
    let _ = row.into_machine().worker("w1".to_string()).build();
    let _ = async {
        vec![JobRow { status: "running" }]
            .into_machines()
            .worker("w1".to_string())
            .build()
            .await
    };
}
