use statum::{machine, state, transition};

mod task {
    use super::*;

    #[state]
    #[derive(Debug)]
    pub enum State {
        Running(Progress),
        Completed,
    }

    #[derive(Debug)]
    pub struct Progress {
        pub ticks: u32,
    }

    #[machine]
    #[derive(Debug)]
    pub struct Machine<State> {}

    #[transition]
    impl Machine<Running> {
        pub fn tick(self) -> Machine<Running> {
            self.transition_map(|progress: Progress| Progress {
                ticks: progress.ticks + 1,
            })
        }

        pub fn complete(self) -> Machine<Completed> {
            self.transition()
        }
    }
}

mod workflow {
    use super::*;

    #[state]
    #[derive(Debug)]
    pub enum State {
        NotStarted,
        InProgress(InProgressData),
        Finished,
    }

    #[derive(Debug)]
    pub struct InProgressData {
        pub owner: String,
        pub task: task::Machine<task::Running>,
    }

    #[machine]
    #[derive(Debug)]
    pub struct Machine<State> {}

    #[transition]
    impl Machine<NotStarted> {
        pub fn start(
            self,
            owner: String,
            task: task::Machine<task::Running>,
        ) -> Machine<InProgress> {
            self.transition_with(InProgressData { owner, task })
        }
    }

    #[transition]
    impl Machine<InProgress> {
        pub fn finish(self) -> Machine<Finished> {
            self.transition_map_child(|task, context| {
                let _ = task.complete();
                println!("Completed child workflow for {}", context.owner);
            })
        }
    }
}

pub fn run() {
    use workflow::machine::ChildExt as _;

    let task_machine = task::Machine::<task::Running>::builder()
        .state_data(task::Progress { ticks: 0 })
        .build();

    let workflow_machine = workflow::Machine::<workflow::NotStarted>::builder().build();

    let workflow_machine = workflow_machine.start("alice".to_string(), task_machine);
    let workflow_machine = workflow_machine.map_child(|task| task.tick());

    println!("Child ticks: {}", workflow_machine.child().state_data.ticks);

    let _finished = workflow_machine.finish();
}
