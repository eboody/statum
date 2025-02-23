use statum::{machine, state, transition};
use tokio::time::{sleep, Duration};

// Example 1: Hierarchical FSMs (Sub-State Machines)

pub mod task {
    use super::*;

    #[state]
    #[derive(Debug)]
    pub enum State {
        Idle,
        Running,
        Completed,
    }

    #[machine]
    #[derive(Debug)]
    pub struct Machine<TaskState> {}
}

pub mod workflow {
    use super::*;

    #[state]
    #[derive(Debug)]
    pub enum State {
        NotStarted,
        InProgress(TaskMachine<Running>),
        Finished,
    }

    #[machine]
    #[derive(Debug)]
    pub struct Machine<State> {}

    #[transition]
    impl Machine<NotStarted> {
        pub fn start(
            self,
            running_task_machine: task::Machine<task::Running>,
        ) -> Machine<InProgress> {
            self.transition_with(running_task_machine)
        }
    }
}

fn main() {
    let task_machine = TaskMachine::<Running>::builder().build();

    let workflow_machine = workflow::Machine::<workflow::NotStarted>::builder().build();

    let workflow_machine = workflow_machine.start(task_machine);

    println!("Task State: {:?}", &workflow_machine.state_data);
}
