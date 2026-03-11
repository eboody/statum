#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};
pub use bon;
use bon::builder as _;
use statum_macros::{machine, state, transition};

#[state]
enum State {
    A,
    B,
}

#[machine]
struct Machine<State> {}

struct Ghost;

#[transition]
impl Machine<A> {
    fn to_ghost(self) -> Machine<Ghost> {
        self.transition()
    }
}

fn main() {}
