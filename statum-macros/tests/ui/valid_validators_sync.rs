use statum::{machine, state, validators}; use statum::Error;

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
    fn is_draft(&self) -> Result<(), Error> {
        let _ = name;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(Error::InvalidState)
        }
    }

    fn is_in_progress(&self) -> Result<Progress, Error> {
        let _ = name;
        if self.status == "progress" {
            Ok(Progress { percent: 0 })
        } else {
            Err(Error::InvalidState)
        }
    }

    fn is_done(&self) -> Result<(), Error> {
        let _ = name;
        if self.status == "done" {
            Ok(())
        } else {
            Err(Error::InvalidState)
        }
    }
}

fn main() {
    let row = DbRow { status: "draft" };
    let _ = row.machine_builder().name("todo".to_string()).build();
}
