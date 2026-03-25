#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state};

#[state]
enum WorkflowState {
    Draft,
    Review(String),
}

#[machine]
struct Workflow<WorkflowState, Context, const VERSION: usize> {
    ctx: Context,
    slots: [u8; VERSION],
}

macro_rules! inspect_validator_contract {
    (
        machine = Workflow,
        state_family = WorkflowState,
        state_trait = WorkflowStateTrait,
        machine_module = workflow,
        machine_vis = $machine_vis:vis,
        extra_generics = {
            params = [
                { Context },
                { const VERSION : usize }
            ],
            args = [
                { Context },
                { VERSION }
            ],
            where_predicates = [],
        },
        fields = [
            { name = ctx, ty = $ctx_ty:ty },
            { name = slots, ty = $slots_ty:ty }
        ],
        variants = [
            { marker = Draft, validator = is_draft, data = $draft_data:ty, has_data = false },
            { marker = Review, validator = is_review, data = $review_data:ty, has_data = true }
        ],
    ) => {
        fn __assert_ctx_field<Context, const VERSION: usize>() {
            let _: core::option::Option<Context> = core::option::Option::<$ctx_ty>::None;
        }

        fn __assert_slots_field<Context, const VERSION: usize>() {
            let _: core::option::Option<[u8; VERSION]> =
                core::option::Option::<$slots_ty>::None;
        }

        fn __assert_draft_data() {
            let _: core::option::Option<()> = core::option::Option::<$draft_data>::None;
        }

        fn __assert_review_data() {
            let _: core::option::Option<String> = core::option::Option::<$review_data>::None;
        }

        fn __assert_state_trait<T: WorkflowStateTrait>() {}
        fn __assert_some_state<Context, const VERSION: usize>(
            state: workflow::SomeState<Context, VERSION>,
        ) -> workflow::SomeState<Context, VERSION> {
            let _ = stringify!($machine_vis);
            state
        }
    };
}

__statum_visit_workflow_validators!(inspect_validator_contract);

fn main() {}
