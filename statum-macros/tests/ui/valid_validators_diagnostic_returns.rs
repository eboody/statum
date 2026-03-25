#![allow(unused_imports)]
extern crate self as statum;
pub use statum_macros::__statum_emit_validator_methods_impl;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    Rejection, Result, StateDescriptor, StateMarker, TransitionDescriptor, UnitState, Validation,
};

use statum_macros::{machine, state, validators};

#[state]
pub enum TaskState {
    Draft,
    InProgress(Progress),
    Done,
}

pub struct Progress {
    percent: u8,
}

#[machine]
pub struct TaskMachine<TaskState> {
    name: String,
}

pub struct DbRow {
    status: &'static str,
}

#[validators(TaskMachine)]
impl DbRow {
    fn is_draft(&self) -> statum::Validation<()> {
        let _ = name;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum::Rejection::new("wrong_status").with_message("expected draft"))
        }
    }

    fn is_in_progress(&self) -> core::result::Result<Progress, statum::Rejection> {
        let _ = name;
        if self.status == "progress" {
            Ok(Progress { percent: 0 })
        } else {
            Err(statum::Rejection::new("wrong_status").with_message("expected progress"))
        }
    }

    fn is_done(&self) -> statum::Result<()> {
        let _ = name;
        if self.status == "done" {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }
}

fn main() {
    let row = DbRow { status: "draft" };
    let report = row.into_machine().name("todo".to_string()).build_report();
    assert_eq!(report.attempts.len(), 1);
    assert!(report.result.is_ok());
}
