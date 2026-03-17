#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState,
};
pub use bon;
use statum_macros::machine;
use bon::builder as _;

enum WorkflowState {
    Draft,
}

#[machine]
struct WorkflowMachine<WorkflowState> {}

fn main() {}
