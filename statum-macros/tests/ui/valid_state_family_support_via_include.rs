#![allow(unused_imports)]
extern crate self as statum;
pub use statum_macros::__statum_emit_validator_methods_impl;
pub use statum_core::__private;
pub use statum_core::{StateMarker, UnitState};

use statum_macros::state;

include!("support/generated_state_family_item.rs");

macro_rules! inspect_family {
    (
        family = IncludedState,
        state_trait = IncludedStateTrait,
        uninitialized = UninitializedIncludedState,
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
                marker = Published,
                is_fn = is_published,
                data = (),
                rust_name = "Published",
                has_data = false,
                has_presentation = false,
                has_metadata = false,
                presentation = { label = None, description = None, metadata = (()) }
            }
        ],
    ) => {
        const _: [(); 2] = [(); <IncludedState as statum::__private::StateFamily>::VARIANT_COUNT];
        const _: [(); 0] = [(); <Draft as statum::__private::StateFamilyMember>::HAS_DATA as usize];
        const _: [(); 0] = [(); <Published as statum::__private::StateFamilyMember>::HAS_DATA as usize];
    };
}

__statum_visit_included_state_family!(inspect_family);

fn main() {
    let _ = core::mem::size_of::<UninitializedIncludedState>();
}
