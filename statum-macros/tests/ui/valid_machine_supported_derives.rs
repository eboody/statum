#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state, transition};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
struct ReviewPayload {
    priority: u8,
}

#[state]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
enum WorkflowState {
    Draft,
    Review(ReviewPayload),
}

#[machine]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
struct Workflow<WorkflowState> {
    owner: u8,
}

#[transition]
impl Workflow<Draft> {
    fn submit(self, payload: ReviewPayload) -> Workflow<Review> {
        self.transition_with(payload)
    }
}

#[state]
#[derive(Default)]
enum ToggleState {
    On,
}

#[machine]
#[derive(Default)]
struct Switch<ToggleState> {
    owner: u8,
}

fn main() {
    use core::hash::{Hash as _, Hasher as _};

    let draft = Workflow::<Draft>::builder().owner(7).build();
    let _copied = draft;
    let _cloned = draft.clone();
    let review_a = draft.submit(ReviewPayload { priority: 1 });
    let review_b = Workflow::<Review>::builder()
        .state_data(ReviewPayload { priority: 2 })
        .owner(7)
        .build();
    let _ = format!("{review_a:?}");
    let _ = review_a == review_b;
    let _ = review_a < review_b;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    review_a.hash(&mut hasher);
    let _ = hasher.finish();

    let switch: Switch<On> = Default::default();
    assert_eq!(switch.owner, 0);
}
