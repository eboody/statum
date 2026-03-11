#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::{CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};
pub use bon;

use bon::builder as _;
use statum_macros::{machine, state, transition};

#[state]
enum FlowState {
    Draft,
    Done,
}

#[machine]
struct AlphaMachine<FlowState> {}

#[transition]
impl AlphaMachine<Draft> {
    fn finish(self) -> AlphaMachine<Done> {
        self.transition()
    }
}

#[machine]
struct BetaMachine<FlowState> {}

#[transition]
impl BetaMachine<Draft> {
    fn finish(self) -> BetaMachine<Done> {
        self.transition()
    }
}

fn main() {
    let alpha = AlphaMachine::<Draft>::builder().build();
    let _ = alpha.finish();

    let beta = BetaMachine::<Draft>::builder().build();
    let _ = beta.finish();
}
