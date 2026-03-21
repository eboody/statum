#![allow(dead_code)]

use statum::{
    machine, state, transition, MachineIntrospection, MachinePresentation,
    MachinePresentationDescriptor, MachineStateIdentity, MachineTransitionRecorder,
    StatePresentation, TransitionPresentation,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Intake,
    Review,
    Decision,
    Output,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FlowMachineMeta {
    phase: Phase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FlowStateMeta {
    phase: Phase,
    source_term: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FlowTransitionMeta {
    phase: Phase,
    branching: bool,
}

#[state]
enum SharedState {
    Draft,
    Review,
    Accepted,
    Rejected,
    Published,
}

#[machine]
struct Flow<SharedState> {}

#[transition]
impl Flow<Draft> {
    fn submit(self) -> Flow<Review> {
        self.transition()
    }
}

#[transition]
impl Flow<Review> {
    fn maybe_decide(self) -> Option<Result<Flow<Accepted>, Flow<Rejected>>> {
        if true {
            Some(Ok(self.accept()))
        } else {
            Some(Err(self.reject()))
        }
    }

    fn accept(self) -> Flow<Accepted> {
        self.transition()
    }

    fn reject(self) -> Flow<Rejected> {
        self.transition()
    }
}

#[transition]
impl Flow<Accepted> {
    fn explain(self) -> Flow<Published> {
        self.transition()
    }
}

#[transition]
impl Flow<Rejected> {
    fn explain(self) -> Flow<Draft> {
        self.transition()
    }
}

#[machine]
struct AlphaMachine<SharedState> {}

#[transition]
impl AlphaMachine<Draft> {
    fn finish(self) -> AlphaMachine<Published> {
        self.transition()
    }
}

#[machine]
struct BetaMachine<SharedState> {}

#[transition]
impl BetaMachine<Draft> {
    fn finish(self) -> BetaMachine<Rejected> {
        self.transition()
    }
}

static FLOW_PRESENTATION: MachinePresentation<
    flow::StateId,
    flow::TransitionId,
    FlowMachineMeta,
    FlowStateMeta,
    FlowTransitionMeta,
> = MachinePresentation {
    machine: Some(MachinePresentationDescriptor {
        label: Some("Flow"),
        description: Some("Example consumer-owned presentation metadata."),
        metadata: FlowMachineMeta {
            phase: Phase::Intake,
        },
    }),
    states: &[
        StatePresentation {
            id: flow::StateId::Draft,
            label: Some("Draft"),
            description: Some("Initial work before submission."),
            metadata: FlowStateMeta {
                phase: Phase::Intake,
                source_term: "draft",
            },
        },
        StatePresentation {
            id: flow::StateId::Review,
            label: Some("Review"),
            description: Some("Work is under review."),
            metadata: FlowStateMeta {
                phase: Phase::Review,
                source_term: "review",
            },
        },
        StatePresentation {
            id: flow::StateId::Accepted,
            label: Some("Accepted"),
            description: Some("Review approved the work."),
            metadata: FlowStateMeta {
                phase: Phase::Decision,
                source_term: "accepted",
            },
        },
        StatePresentation {
            id: flow::StateId::Rejected,
            label: Some("Rejected"),
            description: Some("Review rejected the work."),
            metadata: FlowStateMeta {
                phase: Phase::Decision,
                source_term: "rejected",
            },
        },
        StatePresentation {
            id: flow::StateId::Published,
            label: Some("Published"),
            description: Some("Work has been published."),
            metadata: FlowStateMeta {
                phase: Phase::Output,
                source_term: "published",
            },
        },
    ],
    transitions: &[
        TransitionPresentation {
            id: flow::TransitionId::SubmitFromDraft,
            label: Some("Submit"),
            description: Some("Move draft work into review."),
            metadata: FlowTransitionMeta {
                phase: Phase::Review,
                branching: false,
            },
        },
        TransitionPresentation {
            id: flow::TransitionId::MaybeDecideFromReview,
            label: Some("Validate"),
            description: Some("Choose whether the reviewed work is accepted or rejected."),
            metadata: FlowTransitionMeta {
                phase: Phase::Decision,
                branching: true,
            },
        },
        TransitionPresentation {
            id: flow::TransitionId::AcceptFromReview,
            label: Some("Accept"),
            description: Some("Approve the work."),
            metadata: FlowTransitionMeta {
                phase: Phase::Decision,
                branching: false,
            },
        },
        TransitionPresentation {
            id: flow::TransitionId::RejectFromReview,
            label: Some("Reject"),
            description: Some("Reject the work."),
            metadata: FlowTransitionMeta {
                phase: Phase::Decision,
                branching: false,
            },
        },
        TransitionPresentation {
            id: flow::TransitionId::ExplainFromAccepted,
            label: Some("Publish"),
            description: Some("Move accepted work into published."),
            metadata: FlowTransitionMeta {
                phase: Phase::Output,
                branching: false,
            },
        },
        TransitionPresentation {
            id: flow::TransitionId::ExplainFromRejected,
            label: Some("Rework"),
            description: Some("Loop rejected work back to draft."),
            metadata: FlowTransitionMeta {
                phase: Phase::Output,
                branching: false,
            },
        },
    ],
};

#[test]
fn graph_exposes_exact_transition_sites() {
    let graph = <Flow<Review> as MachineIntrospection>::GRAPH;

    assert_eq!(
        <Flow<Review> as MachineStateIdentity>::STATE_ID,
        flow::StateId::Review
    );
    assert!(graph.state(flow::StateId::Accepted).is_some());
    assert!(!graph.state(flow::StateId::Published).unwrap().has_data);

    let review_methods = graph
        .transitions_from(flow::StateId::Review)
        .map(|transition| transition.method_name)
        .collect::<Vec<_>>();
    assert_eq!(review_methods, vec!["maybe_decide", "accept", "reject"]);

    let maybe_decide = graph
        .transition_from_method(flow::StateId::Review, "maybe_decide")
        .unwrap();
    assert_eq!(maybe_decide.id, flow::TransitionId::MaybeDecideFromReview);
    assert_eq!(
        graph.legal_targets(maybe_decide.id).unwrap(),
        &[flow::StateId::Accepted, flow::StateId::Rejected]
    );

    let accepted_explain = graph
        .transition_from_method(flow::StateId::Accepted, "explain")
        .unwrap();
    assert_eq!(accepted_explain.id, flow::TransitionId::ExplainFromAccepted);
    let rejected_explain = graph
        .transition_from_method(flow::StateId::Rejected, "explain")
        .unwrap();
    assert_eq!(rejected_explain.id, flow::TransitionId::ExplainFromRejected);
    assert_eq!(graph.transitions_named("explain").count(), 2);
}

#[test]
fn graph_collection_is_scoped_per_machine() {
    let alpha_graph = <AlphaMachine<Draft> as MachineIntrospection>::GRAPH;
    let beta_graph = <BetaMachine<Draft> as MachineIntrospection>::GRAPH;

    let alpha_finish = alpha_graph
        .transition(alpha_machine::TransitionId::FinishFromDraft)
        .unwrap();
    assert_eq!(
        alpha_graph.legal_targets(alpha_finish.id).unwrap(),
        &[alpha_machine::StateId::Published]
    );

    let beta_finish = beta_graph
        .transition(beta_machine::TransitionId::FinishFromDraft)
        .unwrap();
    assert_eq!(
        beta_graph.legal_targets(beta_finish.id).unwrap(),
        &[beta_machine::StateId::Rejected]
    );
}

#[test]
fn runtime_transition_recording_joins_to_static_metadata() {
    let event = Flow::<Review>::try_record_transition_to::<Flow<Accepted>>(
        flow::TransitionId::MaybeDecideFromReview,
    )
    .unwrap();
    let graph = <Flow<Review> as MachineIntrospection>::GRAPH;
    let transition = graph.transition(event.transition).unwrap();

    assert_eq!(event.machine, graph.machine);
    assert_eq!(event.from, flow::StateId::Review);
    assert_eq!(event.chosen, flow::StateId::Accepted);
    assert_eq!(transition.from, event.from);
    assert!(transition.to.contains(&event.chosen));
}

#[test]
fn runtime_transition_recording_rejects_illegal_runtime_join() {
    assert!(Flow::<Review>::try_record_transition(
        flow::TransitionId::SubmitFromDraft,
        flow::StateId::Accepted,
    )
    .is_none());
    assert!(Flow::<Review>::try_record_transition_to::<Flow<Published>>(
        flow::TransitionId::MaybeDecideFromReview,
    )
    .is_none());
}

#[test]
fn consumer_owned_presentation_metadata_joins_with_graph_and_runtime_event() {
    let graph = <Flow<Review> as MachineIntrospection>::GRAPH;
    let event = Flow::<Review>::try_record_transition_to::<Flow<Accepted>>(
        flow::TransitionId::MaybeDecideFromReview,
    )
    .unwrap();

    assert_eq!(
        FLOW_PRESENTATION.machine,
        Some(MachinePresentationDescriptor {
            label: Some("Flow"),
            description: Some("Example consumer-owned presentation metadata."),
            metadata: FlowMachineMeta {
                phase: Phase::Intake,
            },
        })
    );
    assert_eq!(
        event.transition_in(graph).unwrap().method_name,
        "maybe_decide"
    );
    assert_eq!(
        FLOW_PRESENTATION
            .transition(event.transition)
            .unwrap()
            .label,
        Some("Validate")
    );
    assert_eq!(
        FLOW_PRESENTATION.state(event.chosen).unwrap().metadata,
        FlowStateMeta {
            phase: Phase::Decision,
            source_term: "accepted",
        }
    );
    assert_eq!(
        graph.legal_targets(event.transition).unwrap(),
        &[flow::StateId::Accepted, flow::StateId::Rejected]
    );
}
