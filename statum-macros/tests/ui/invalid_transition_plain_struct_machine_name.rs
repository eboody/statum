#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState,
};
// Legacy compatibility import removed.
use statum_macros::{state, transition};
// Builder methods are inherent.

#[state]
enum State {
    A,
    B,
}

struct Machine<State>(core::marker::PhantomData<State>);

#[transition]
impl Machine<A> {
    fn to_b(self) -> Machine<B> {
        Machine(core::marker::PhantomData)
    }
}

fn main() {}
