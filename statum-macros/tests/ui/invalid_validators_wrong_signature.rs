use statum::{machine, state, validators}; use statum::Error;

#[state]
enum TaskState {
    Draft,
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
    fn is_draft(&self, extra: u8) -> Result<(), Error> {
        let _ = name;
        let _ = extra;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(Error::InvalidState)
        }
    }
}
