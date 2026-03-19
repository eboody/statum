#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};
// Legacy compatibility import removed.
use statum_macros::{machine, state, transition};
// Builder methods are inherent.

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