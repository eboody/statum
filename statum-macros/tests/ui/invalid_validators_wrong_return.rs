#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::Error;
pub use bon;
use statum_macros::{machine, state, validators};
use bon::builder as _;

#[state]
enum TaskState {
    Draft,
    InProgress(Progress),
}

struct Progress {
    count: u8,
}

#[machine]
struct TaskMachine<TaskState> {
    name: String,
}

struct DbRow {
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

    fn is_in_progress(&self) -> Result<(), statum_core::Error> {
        let _ = name;
        if self.status == "progress" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}