#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};
pub use bon;
use statum_macros::{machine, state};
use bon::builder as _;

#[state]
enum WorkflowState {
    Draft,
}

#[machine]
struct Workflow<WorkflowState, Context> {
    marker: core::marker::PhantomData<Context>,
}

fn main() {}
