#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};
pub use bon;
use statum_macros::{machine, state, transition};
use bon::builder as _;

#[state]
enum State {
    A,
    B,
}

#[machine]
struct Machine<State> {}

#[transition]
impl Machine<A> {
    fn to_b(_value: u64) -> Machine<B> {
        unimplemented!()
    }
}