#![allow(unused_imports)]
extern crate self as statum;
pub use bon;
pub use statum_core::{CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};
use bon::builder as _;
use statum_macros::{machine, state, transition};

#[state]
enum State {
    A,
    B,
}

#[machine]
struct Machine<State> {}

#[transition]
impl Machine<A> {
    fn to_b(self) -> Option<core::result::Result<Machine<B>, statum_core::Error>> {
        Some(Ok(self.transition()))
    }
}

fn main() {
    let machine = Machine::<A>::builder().build();
    let _ = machine.to_b();
}
