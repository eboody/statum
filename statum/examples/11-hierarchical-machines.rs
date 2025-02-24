use statum::{machine, state, transition};

mod task {
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
    pub struct Machine<State> {}
}

mod workflow {
    use super::*;

    #[state]
    #[derive(Debug)]
    pub enum State {
        NotStarted,
        InProgress(task::Machine<task::Running>),
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
    let task_machine = task::Machine::<task::Running>::builder().build();

    let workflow_machine = workflow::Machine::<workflow::NotStarted>::builder().build();

    let workflow_machine = workflow_machine.start(task_machine);

    println!("Task State: {:?}", &workflow_machine.state_data);
}
