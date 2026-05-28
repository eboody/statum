#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, RebuildAttempt,
    RebuildReport, StateMarker, UnitState,
};

use statum_macros::{machine, state, transition};

#[state]
enum FlowState {
    Draft,
    Review,
    Published,
}

#[machine]
struct Flow<FlowState> {
    id: u64,
}

#[transition]
impl Flow<Draft> {
    fn submit(self) -> Flow<Review> {
        self.transition()
    }
}

#[transition]
impl Flow<Review> {
    fn publish(self) -> Flow<Published> {
        self.transition()
    }
}

fn main() {
    let draft = Flow::<Draft>::builder().id(7).build();
    let published = draft.submit().publish();
    assert_eq!(published.id, 7);
}
