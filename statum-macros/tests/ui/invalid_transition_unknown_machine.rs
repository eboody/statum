#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};

use statum_macros::{machine, state, transition};


#[state]
enum State {
    A,
    B,
}

#[machine]
struct Machine<State> {}

struct DoesNotExist<S>(core::marker::PhantomData<S>);

#[transition]
impl DoesNotExist<A> {
    fn to_b(self) -> DoesNotExist<B> {
        unimplemented!()
    }
}

fn main() {}
