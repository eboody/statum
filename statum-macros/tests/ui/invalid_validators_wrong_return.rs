use statum::{machine, state, validators}; use statum::Error;

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
    fn is_draft(&self) -> Result<(), Error> {
        let _ = name;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(Error::InvalidState)
        }
    }

    fn is_in_progress(&self) -> Result<(), Error> {
        let _ = name;
        if self.status == "progress" {
            Ok(())
        } else {
            Err(Error::InvalidState)
        }
    }
}
