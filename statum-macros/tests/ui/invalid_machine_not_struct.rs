#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};
pub use bon;
use statum_macros::machine;
use bon::builder as _;

#[machine]
enum NotAStruct {
    Variant,
}