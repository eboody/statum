use statum::{machine, state, validators}; use statum::Error;

#[state]
enum TaskState {
    Draft,
    Done,
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
}
