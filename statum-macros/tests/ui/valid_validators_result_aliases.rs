#![allow(unused_imports)]
extern crate self as statum;
pub use bon;
pub use statum_core::{CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};
pub type Result<T> = core::result::Result<T, Error>;

use bon::builder as _;
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
    fn is_draft(&self) -> statum::Result<()> {
        let _ = name;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_in_progress(&self) -> core::result::Result<Progress, statum::Error> {
        let _ = name;
        if self.status == "progress" {
            Ok(Progress { percent: 0 })
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_done(&self) -> std::result::Result<(), statum::Error> {
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
    let machine = row.into_machine().name("todo".to_string()).build().unwrap();
    let _ = machine;
}
