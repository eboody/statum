use statum::{machine, state, validators};

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
impl DbRow {}
