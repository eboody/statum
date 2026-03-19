#![allow(unused_imports)]
extern crate self as statum;
// Legacy compatibility import removed.
pub use statum_core::{CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, StateMarker, UnitState};

// Builder methods are inherent.
use statum_macros::state;

#[state]
enum TaskState {
    Draft,
    Review(String),
}

fn assert_state_variant<T: StateVariant>() {}

fn assert_requires_state_data<T: RequiresStateData>() {}

fn assert_does_not_require_state_data<T: DoesNotRequireStateData>() {}

fn main() {
    assert_state_variant::<Draft>();
    assert_requires_state_data::<Review>();
    assert_does_not_require_state_data::<Draft>();
}
