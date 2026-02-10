#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::Error;
pub use bon;
use statum_macros::{machine, state, validators};
use bon::builder as _;

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
    fn is_draft(&self) -> Result<(), statum_core::Error> {
        let _ = name;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    fn is_in_progress(&self) -> Result<Progress, statum_core::Error> {
        let _ = name;
        if self.status == "progress" {
            Ok(Progress { percent: 0 })
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    fn is_done(&self) -> Result<(), statum_core::Error> {
        let _ = name;
        if self.status == "done" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}

fn main() {
    let row = DbRow { status: "draft" };
    let machine = row.into_machine().name("todo".to_string()).build().unwrap();

    match machine {
        task_machine::State::Draft(_machine) => {}
        task_machine::State::InProgress(_machine) => {}
        task_machine::State::Done(_machine) => {}
    }
}
