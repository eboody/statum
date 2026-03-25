#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::{DataState, StateMarker, UnitState};

use statum_macros::state;

#[state]
pub enum WorkflowState {
    Draft,
    Review(String),
    Published {
        reviewer: String,
    },
}

fn assert_unit_member<S>()
where
    S: statum::__private::StateFamilyMember<Data = ()> + statum::UnitState,
{
}

fn assert_data_member<S, D>()
where
    S: statum::__private::StateFamilyMember<Data = D> + statum::DataState,
{
}

macro_rules! inspect_family {
    (
        family = $family:ident,
        state_trait = $state_trait:ident,
        uninitialized = $uninitialized:ident,
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
            },
            {
                marker = Published,
                is_fn = is_published,
                data = PublishedData,
                rust_name = "Published",
                has_data = true,
                has_presentation = false,
                has_metadata = false,
                presentation = { label = None, description = None, metadata = (()) }
            }
        ],
    ) => {
        const _: [(); 3] = [(); <$family as statum::__private::StateFamily>::VARIANT_COUNT];
        const _: [(); 0] = [(); <Draft as statum::__private::StateFamilyMember>::HAS_DATA as usize];
        const _: [(); 1] = [(); <Review as statum::__private::StateFamilyMember>::HAS_DATA as usize];
        const _: [(); 1] = [(); <Published as statum::__private::StateFamilyMember>::HAS_DATA as usize];

        fn __assert_state_trait<T: $state_trait>() {}

        fn __assert_uninitialized<T>()
        where
            T: statum::__private::StateFamilyMember<Data = ()> + statum::UnitState,
        {
        }
    };
}

__statum_visit_workflow_state_family!(inspect_family);

fn main() {
    assert_unit_member::<Draft>();
    assert_data_member::<Review, String>();
    assert_data_member::<Published, PublishedData>();
    let _ = core::mem::size_of::<UninitializedWorkflowState>();
    let _ = <Draft as statum::__private::StateFamilyMember>::RUST_NAME;
    let _ = <WorkflowState as statum::__private::StateFamily>::NAME;
}
