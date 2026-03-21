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

mod workflow {
    use super::*;

    #[state]
    pub enum State {
        InProgress(InProgressData),
    }

    pub struct InProgressData {
        pub primary: crate::task::Machine<crate::task::Running>,
        pub secondary: crate::task::Machine<crate::task::Running>,
    }

    #[machine]
    pub struct Machine<State> {}
}

fn main() {}
