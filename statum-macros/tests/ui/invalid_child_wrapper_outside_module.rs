#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};

use statum_macros::{machine, state};

mod task {
    use super::*;

    #[state]
    pub enum State {
        Running,
    }

    #[machine]
    pub struct Machine<State> {}
}

mod shared {
    pub struct InProgressData {
        pub owner: String,
        pub task: crate::task::Machine<crate::task::Running>,
    }
}

mod workflow {
    use super::*;

    #[state]
    pub enum State {
        NotStarted,
        InProgress(crate::shared::InProgressData),
        Finished,
    }

    #[machine]
    pub struct Machine<State> {}

}

fn main() {}
