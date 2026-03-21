#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};

use statum_macros::{machine, state, transition};

mod task {
    use super::*;

    #[state]
    pub enum State {
        Running(RunningData),
        Completed,
    }

    pub struct RunningData {
        pub attempts: u32,
    }

    #[machine]
    pub struct Machine<State> {}

    #[transition]
    impl Machine<Running> {
        pub fn tick(self) -> Machine<Running> {
            self.transition_map(|running: RunningData| RunningData {
                attempts: running.attempts + 1,
            })
        }

        pub fn complete(self) -> Machine<Completed> {
            self.transition()
        }
    }
}

mod direct_parent {
    use super::*;

    #[state]
    pub enum State {
        NotStarted,
        InProgress(crate::task::Machine<crate::task::Running>),
        Finished,
    }

    #[machine]
    pub struct Machine<State> {}

    #[transition]
    impl Machine<NotStarted> {
        pub fn start(
            self,
            child: crate::task::Machine<crate::task::Running>,
        ) -> Machine<InProgress> {
            self.transition_with(child)
        }
    }

    #[transition]
    impl Machine<InProgress> {
        pub fn finish(self) -> Machine<Finished> {
            self.transition_map_child(|child, ()| {
                let _ = child.complete();
            })
        }
    }
}

mod wrapper_parent {
    use super::*;

    #[state]
    pub enum State {
        NotStarted,
        InProgress(InProgressData),
        Finished,
    }

    pub struct InProgressData {
        pub owner: String,
        pub task: crate::task::Machine<crate::task::Running>,
    }

    #[machine]
    pub struct Machine<State> {
        pub id: u64,
    }

    #[transition]
    impl Machine<NotStarted> {
        pub fn start(
            self,
            owner: String,
            task: crate::task::Machine<crate::task::Running>,
        ) -> Machine<InProgress> {
            self.transition_with(InProgressData { owner, task })
        }
    }

    #[transition]
    impl Machine<InProgress> {
        pub fn finish(self) -> Machine<Finished> {
            self.transition_map_child(|task, context| {
                let _ = context.owner;
                let _ = task.complete();
            })
        }
    }
}

fn main() {
    use direct_parent::machine::ChildExt as _;
    use wrapper_parent::machine::ChildExt as _;

    let direct_child = task::Machine::<task::Running>::builder()
        .state_data(task::RunningData { attempts: 0 })
        .build();
    let direct_parent = direct_parent::Machine::<direct_parent::NotStarted>::builder().build();
    let direct_parent = direct_parent.start(direct_child);
    let _ = direct_parent.child().state_data.attempts;
    let _ = direct_parent.finish();

    let wrapper_child = task::Machine::<task::Running>::builder()
        .state_data(task::RunningData { attempts: 1 })
        .build();
    let wrapper_parent = wrapper_parent::Machine::<wrapper_parent::NotStarted>::builder()
        .id(7)
        .build();
    let wrapper_parent = wrapper_parent.start("ada".to_string(), wrapper_child);
    let _ = wrapper_parent.child().state_data.attempts;
    let wrapper_parent = wrapper_parent.map_child(|child| child.tick());
    let _ = wrapper_parent.child().state_data.attempts;
    let _ = wrapper_parent.finish();
}
