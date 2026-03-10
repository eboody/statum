#![allow(unused_imports)]
extern crate self as statum;
pub use bon;
pub use statum_core::Error;
pub type Result<T> = core::result::Result<T, Error>;

use bon::builder as _;
use statum_macros::{machine, state, validators};

#[state]
pub enum TaskState {
    Draft,
    Batched(Vec<Progress>),
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
    fn is_draft(&self) -> Result<()> {
        let _ = name;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_batched(&self) -> core::result::Result<Vec<Progress>, statum::Error> {
        let _ = name;
        if self.status == "batched" {
            Ok(vec![Progress { percent: 100 }])
        } else {
            Err(statum::Error::InvalidState)
        }
    }
}

fn main() {
    let row = DbRow { status: "batched" };
    let machine = row.into_machine().name("todo".to_string()).build().unwrap();
    let _ = machine;
}
