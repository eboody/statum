#![allow(unused_imports)]
extern crate self as statum;
pub use bon;
pub use statum_core::Error;
pub type Result<T> = core::result::Result<T, Error>;

use bon::builder as _;
use statum_macros::{machine, state, validators};

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
    fn is_draft(&self) -> statum::Result<()> {
        let _ = name;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_in_progress(&self) -> statum::Result<()> {
        let _ = name;
        if self.status == "progress" {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }
}
