#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState,
};
pub use bon;
use statum_macros::{state, transition};
use bon::builder as _;

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
