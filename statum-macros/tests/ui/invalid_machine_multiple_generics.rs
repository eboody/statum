#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};
// Legacy compatibility import removed.
use statum_macros::{machine, state};
// Builder methods are inherent.

#[state]
enum WorkflowState {
    Draft,
}

#[machine]
struct Workflow<WorkflowState, Context> {
    marker: core::marker::PhantomData<Context>,
}

fn main() {}
