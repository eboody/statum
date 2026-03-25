#![allow(unused_imports)]
extern crate self as statum;
pub use statum_macros::__statum_emit_validator_methods_impl;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state, transition, validators};

#[state]
enum WorkflowState {
    Draft,
    Review(String),
    Published,
}

#[machine]
struct Workflow<WorkflowState, Context, const VERSION: usize> {
    ctx: Context,
    slots: [u8; VERSION],
}

#[transition]
impl<Context, const VERSION: usize> Workflow<Draft, Context, VERSION> {
    fn submit(self, reviewer: String) -> Workflow<Review, Context, VERSION> {
        self.transition_with(reviewer)
    }
}

#[transition]
impl<Context, const VERSION: usize> Workflow<Review, Context, VERSION> {
    fn publish(self) -> Workflow<Published, Context, VERSION> {
        self.transition()
    }
}

struct Row {
    status: &'static str,
    reviewer: Option<&'static str>,
}

#[validators(Workflow)]
impl Row {
    fn is_draft(&self) -> Result<(), statum_core::Error> {
        let _ = &ctx;
        let _ = &slots;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    fn is_review(&self) -> Result<String, statum_core::Error> {
        let _ = &ctx;
        let _ = &slots;
        if self.status == "review" {
            Ok(self.reviewer.expect("reviewer").to_owned())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }

    fn is_published(&self) -> Result<(), statum_core::Error> {
        let _ = &ctx;
        let _ = &slots;
        if self.status == "published" {
            Ok(())
        } else {
            Err(statum_core::Error::InvalidState)
        }
    }
}

fn main() {
    use workflow::IntoMachinesExt as _;

    let review = Workflow::<Draft, String, 2>::builder()
        .ctx("ctx".to_owned())
        .slots([1, 2])
        .build()
        .submit("alice".to_owned());
    let published = review.publish();
    let _ = (&published.ctx, published.slots);

    let graph = <Workflow<Review, String, 2> as MachineIntrospection>::GRAPH;
    let submit = graph
        .transition_from_method(workflow::StateId::Draft, "submit")
        .unwrap();
    assert_eq!(graph.legal_targets(submit.id).unwrap(), &[workflow::StateId::Review]);

    let rebuilt = Row {
        status: "review",
        reviewer: Some("bob"),
    }
    .into_machine()
    .ctx("rebuilt".to_owned())
    .slots([9, 9])
    .build()
    .unwrap();
    match rebuilt {
        workflow::SomeState::Review(machine) => {
            assert_eq!(machine.ctx, "rebuilt".to_owned());
            assert_eq!(machine.state_data, "bob".to_owned());
            assert_eq!(machine.slots, [9, 9]);
        }
        _ => panic!("expected review state"),
    }

    let batch = vec![Row {
        status: "draft",
        reviewer: None,
    }]
    .into_machines_by(|_| workflow::Fields::<String, 2> {
        ctx: "batch".to_owned(),
        slots: [5, 6],
    })
    .build();
    match batch.into_iter().next().unwrap().unwrap() {
        workflow::SomeState::Draft(machine) => {
            assert_eq!(machine.ctx, "batch".to_owned());
            assert_eq!(machine.slots, [5, 6]);
        }
        _ => panic!("expected draft state"),
    }
}
