#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::{DataState, StateMarker, UnitState};

use statum_macros::state;

macro_rules! declare_workflow_state {
    () => {
        #[state]
        enum WorkflowState {
            Draft,
            Review(String),
        }
    };
}

declare_workflow_state!();

macro_rules! inspect_family {
    (
        family = WorkflowState,
        state_trait = WorkflowStateTrait,
        uninitialized = UninitializedWorkflowState,
        variants = [
            {
                marker = Draft,
                is_fn = is_draft,
                data = (),
                rust_name = "Draft",
                has_data = false,
                has_presentation = false,
                has_metadata = false,
                presentation = { label = None, description = None, metadata = (()) }
            },
            {
                marker = Review,
                is_fn = is_review,
                data = String,
                rust_name = "Review",
                has_data = true,
                has_presentation = false,
                has_metadata = false,
                presentation = { label = None, description = None, metadata = (()) }
            }
        ],
    ) => {
        const _: [(); 2] = [(); <WorkflowState as statum::__private::StateFamily>::VARIANT_COUNT];
        const _: [(); 0] = [(); <Draft as statum::__private::StateFamilyMember>::HAS_DATA as usize];
        const _: [(); 1] = [(); <Review as statum::__private::StateFamilyMember>::HAS_DATA as usize];
    };
}

__statum_visit_workflow_state_family!(inspect_family);

fn main() {
    let _ = <WorkflowState as statum::__private::StateFamily>::NAME;
}
