#![allow(unused_imports)]
extern crate self as statum;
pub use bon;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState,
};
use bon::builder as _;
use statum_macros::machine;

enum WorkflowState {
    Draft,
}

#[machine]
struct WorkflowMachine<WorkflowState> {}

fn main() {}
