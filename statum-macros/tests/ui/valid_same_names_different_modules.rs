#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, StateDescriptor, StateMarker,
    TransitionDescriptor, UnitState,
};
pub use statum_core::Result;


use statum_macros::{machine, state, transition};


mod alpha {
    use super::*;

    #[state]
    pub enum State {
        Off,
        On,
    }

    #[machine]
    pub struct Machine<State> {
        id: u8,
    }

    #[transition]
    impl Machine<Off> {
        pub fn turn_on(self) -> Machine<On> {
            self.transition()
        }
    }
}

mod beta {
    use super::*;

    #[state]
    pub enum State {
        Idle,
        Running,
    }

    #[machine]
    pub struct Machine<State> {
        name: String,
    }

    #[transition]
    impl Machine<Idle> {
        pub fn start(self) -> Machine<Running> {
            self.transition()
        }
    }
}

fn main() {
    let left = alpha::Machine::<alpha::Off>::builder().id(1).build();
    let right = beta::Machine::<beta::Idle>::builder()
        .name("job".to_string())
        .build();

    let _ = left.turn_on();
    let _ = right.start();
}
