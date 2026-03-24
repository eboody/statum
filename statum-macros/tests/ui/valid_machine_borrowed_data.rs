#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state, transition, validators};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReviewPayload<T> {
    reviewer: T,
}

#[state]
enum WorkflowState {
    Draft,
    Review(ReviewPayload<&'static str>),
}

#[machine]
struct Workflow<WorkflowState, Context> {
    ctx: Context,
    audit: &'static str,
}

#[transition]
impl<Context> Workflow<Draft, Context> {
    fn submit(self, reviewer: &'static str) -> Workflow<Review, Context> {
        self.transition_with(ReviewPayload { reviewer })
    }
}

struct Row {
    status: &'static str,
    reviewer: Option<&'static str>,
}

#[validators(Workflow)]
impl Row {
    fn is_draft(&self) -> core::result::Result<(), statum_core::Error> {
        let _ = &ctx;
        let _ = &audit;
        if self.status == "draft" {
            Ok(())
        } else {
            Err(statum::Error::InvalidState)
        }
    }

    fn is_review(
        &self,
    ) -> core::result::Result<ReviewPayload<&'static str>, statum_core::Error> {
        let _ = &ctx;
        let _ = &audit;
        if self.status == "review" {
            Ok(ReviewPayload {
                reviewer: self.reviewer.expect("reviewer"),
            })
        } else {
            Err(statum::Error::InvalidState)
        }
    }
}

fn main() {
    use workflow::IntoMachinesExt as _;

    let review = Workflow::<Draft, &'static str>::builder()
        .ctx("ctx")
        .audit("audit")
        .build()
        .submit("alice");
    assert_eq!(review.ctx, "ctx");
    assert_eq!(review.audit, "audit");
    assert_eq!(review.state_data.reviewer, "alice");

    let graph = <Workflow<Review, &'static str> as MachineIntrospection>::GRAPH;
    let submit = graph
        .transition_from_method(workflow::StateId::Draft, "submit")
        .unwrap();
    assert_eq!(graph.legal_targets(submit.id).unwrap(), &[workflow::StateId::Review]);

    let rebuilt = Row {
        status: "review",
        reviewer: Some("bob"),
    }
    .into_machine()
    .ctx("rebuilt")
    .audit("persisted")
    .build()
    .unwrap();
    match rebuilt {
        workflow::SomeState::Review(machine) => {
            assert_eq!(machine.ctx, "rebuilt");
            assert_eq!(machine.audit, "persisted");
            assert_eq!(machine.state_data.reviewer, "bob");
        }
        _ => panic!("expected review state"),
    }

    let batch = vec![Row {
        status: "draft",
        reviewer: None,
    }]
    .into_machines_by(|_| workflow::Fields::<&'static str> {
        ctx: "batch",
        audit: "batch-audit",
    })
    .build();
    match batch.into_iter().next().unwrap().unwrap() {
        workflow::SomeState::Draft(machine) => {
            assert_eq!(machine.ctx, "batch");
            assert_eq!(machine.audit, "batch-audit");
        }
        _ => panic!("expected draft state"),
    }
}
