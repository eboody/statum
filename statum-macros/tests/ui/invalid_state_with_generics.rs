#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};
use statum_macros::state;

#[state]
enum GenericState<'a, T> {
    Draft(&'a T),
    InProgress(T),
}