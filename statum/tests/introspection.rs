#![allow(dead_code)]

use statum::{
    machine, state, transition, MachineIntrospection, MachinePresentation,
    MachinePresentationDescriptor, MachineStateIdentity, MachineTransitionRecorder,
    StatePresentation, TransitionPresentation, TransitionPresentationInventory,
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

#[state]
enum PresentedState {
    #[present(label = "Queued", description = "Waiting for review.")]
    QueuedPresentation,
    ReviewingPresentation,
    #[present(label = "Done")]
    DonePresentation,
}

#[machine]
#[present(
    label = "Presented Flow",
    description = "Macro-generated presentation metadata."
)]
struct PresentedFlow<PresentedState> {}

#[transition]
impl PresentedFlow<QueuedPresentation> {
    #[present(label = "Start Review", description = "Move queued work into review.")]
    fn start_review(self) -> PresentedFlow<ReviewingPresentation> {
        self.transition()
    }
}

#[transition]
impl PresentedFlow<ReviewingPresentation> {
    #[present(label = "Complete", description = "Finish the workflow.")]
    fn complete(self) -> PresentedFlow<DonePresentation> {
        self.transition()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GeneratedMachineMeta {
    TypedPresentation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GeneratedStateMeta {
    Queued,
    Reviewing,
    Done,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GeneratedTransitionMeta {
    StartReview,
    Complete,
}

#[state]
enum TypedPresentedState {
    #[present(label = "Queued", metadata = GeneratedStateMeta::Queued)]
    QueuedTyped,
    #[present(label = "Reviewing", metadata = GeneratedStateMeta::Reviewing)]
    ReviewingTyped,
    #[present(label = "Done", metadata = GeneratedStateMeta::Done)]
    DoneTyped,
}

#[machine]
#[presentation_types(
    machine = GeneratedMachineMeta,
    state = GeneratedStateMeta,
    transition = GeneratedTransitionMeta,
)]
#[present(
    label = "Typed Presented Flow",
    metadata = GeneratedMachineMeta::TypedPresentation
)]
struct TypedPresentedFlow<TypedPresentedState> {}

#[transition]
impl TypedPresentedFlow<QueuedTyped> {
    #[present(label = "Start Review", metadata = GeneratedTransitionMeta::StartReview)]
    fn start_review_typed(self) -> TypedPresentedFlow<ReviewingTyped> {
        self.transition()
    }
}

#[transition]
impl TypedPresentedFlow<ReviewingTyped> {
    #[present(label = "Complete", metadata = GeneratedTransitionMeta::Complete)]
    fn complete_typed(self) -> TypedPresentedFlow<DoneTyped> {
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
    transitions: TransitionPresentationInventory::new(|| &FLOW_TRANSITIONS),
};

static FLOW_TRANSITIONS: [TransitionPresentation<flow::TransitionId, FlowTransitionMeta>; 6] = [
    TransitionPresentation {
        id: Flow::<Draft>::SUBMIT,
        label: Some("Submit"),
        description: Some("Move draft work into review."),
        metadata: FlowTransitionMeta {
            phase: Phase::Review,
            branching: false,
        },
    },
    TransitionPresentation {
        id: Flow::<Review>::MAYBE_DECIDE,
        label: Some("Validate"),
        description: Some("Choose whether the reviewed work is accepted or rejected."),
        metadata: FlowTransitionMeta {
            phase: Phase::Decision,
            branching: true,
        },
    },
    TransitionPresentation {
        id: Flow::<Review>::ACCEPT,
        label: Some("Accept"),
        description: Some("Approve the work."),
        metadata: FlowTransitionMeta {
            phase: Phase::Decision,
            branching: false,
        },
    },
    TransitionPresentation {
        id: Flow::<Review>::REJECT,
        label: Some("Reject"),
        description: Some("Reject the work."),
        metadata: FlowTransitionMeta {
            phase: Phase::Decision,
            branching: false,
        },
    },
    TransitionPresentation {
        id: Flow::<Accepted>::EXPLAIN,
        label: Some("Publish"),
        description: Some("Move accepted work into published."),
        metadata: FlowTransitionMeta {
            phase: Phase::Output,
            branching: false,
        },
    },
    TransitionPresentation {
        id: Flow::<Rejected>::EXPLAIN,
        label: Some("Rework"),
        description: Some("Loop rejected work back to draft."),
        metadata: FlowTransitionMeta {
            phase: Phase::Output,
            branching: false,
        },
    },
];

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
    let mut review_methods = review_methods;
    review_methods.sort_unstable();
    assert_eq!(review_methods, vec!["accept", "maybe_decide", "reject"]);

    let maybe_decide = graph
        .transition_from_method(flow::StateId::Review, "maybe_decide")
        .unwrap();
    assert_eq!(maybe_decide.id, Flow::<Review>::MAYBE_DECIDE);
    assert_eq!(
        graph.legal_targets(maybe_decide.id).unwrap(),
        &[flow::StateId::Accepted, flow::StateId::Rejected]
    );

    let accepted_explain = graph
        .transition_from_method(flow::StateId::Accepted, "explain")
        .unwrap();
    assert_eq!(accepted_explain.id, Flow::<Accepted>::EXPLAIN);
    let rejected_explain = graph
        .transition_from_method(flow::StateId::Rejected, "explain")
        .unwrap();
    assert_eq!(rejected_explain.id, Flow::<Rejected>::EXPLAIN);
    assert_eq!(graph.transitions_named("explain").count(), 2);
}

#[test]
fn graph_collection_is_scoped_per_machine() {
    let alpha_graph = <AlphaMachine<Draft> as MachineIntrospection>::GRAPH;
    let beta_graph = <BetaMachine<Draft> as MachineIntrospection>::GRAPH;

    let alpha_finish = alpha_graph
        .transition(AlphaMachine::<Draft>::FINISH)
        .unwrap();
    assert_eq!(
        alpha_graph.legal_targets(alpha_finish.id).unwrap(),
        &[alpha_machine::StateId::Published]
    );

    let beta_finish = beta_graph.transition(BetaMachine::<Draft>::FINISH).unwrap();
    assert_eq!(
        beta_graph.legal_targets(beta_finish.id).unwrap(),
        &[beta_machine::StateId::Rejected]
    );
}

#[test]
fn runtime_transition_recording_joins_to_static_metadata() {
    let event =
        Flow::<Review>::try_record_transition_to::<Flow<Accepted>>(Flow::<Review>::MAYBE_DECIDE)
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
    assert!(
        Flow::<Review>::try_record_transition(Flow::<Draft>::SUBMIT, flow::StateId::Accepted,)
            .is_none()
    );
    assert!(Flow::<Review>::try_record_transition_to::<Flow<Published>>(
        Flow::<Review>::MAYBE_DECIDE,
    )
    .is_none());
}

#[test]
fn consumer_owned_presentation_metadata_joins_with_graph_and_runtime_event() {
    let graph = <Flow<Review> as MachineIntrospection>::GRAPH;
    let event =
        Flow::<Review>::try_record_transition_to::<Flow<Accepted>>(Flow::<Review>::MAYBE_DECIDE)
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

#[test]
fn generated_presentation_metadata_joins_with_graph_and_runtime_event() {
    let graph = <PresentedFlow<QueuedPresentation> as MachineIntrospection>::GRAPH;
    let event = PresentedFlow::<QueuedPresentation>::try_record_transition_to::<
        PresentedFlow<ReviewingPresentation>,
    >(PresentedFlow::<QueuedPresentation>::START_REVIEW)
    .unwrap();
    let presentation = &presented_flow::PRESENTATION;

    assert_eq!(presentation.machine.unwrap().label, Some("Presented Flow"));
    assert_eq!(
        presentation
            .state(presented_flow::StateId::QueuedPresentation)
            .unwrap()
            .description,
        Some("Waiting for review.")
    );
    assert_eq!(
        presentation.transition(event.transition).unwrap().label,
        Some("Start Review")
    );
    assert_eq!(
        event.transition_in(graph).unwrap().method_name,
        "start_review"
    );
    assert_eq!(
        graph.legal_targets(event.transition).unwrap(),
        &[presented_flow::StateId::ReviewingPresentation]
    );
}

#[test]
fn typed_generated_presentation_metadata_joins_with_graph_and_runtime_event() {
    let graph = <TypedPresentedFlow<QueuedTyped> as MachineIntrospection>::GRAPH;
    let event = TypedPresentedFlow::<QueuedTyped>::try_record_transition_to::<
        TypedPresentedFlow<ReviewingTyped>,
    >(TypedPresentedFlow::<QueuedTyped>::START_REVIEW_TYPED)
    .unwrap();
    let presentation = &typed_presented_flow::PRESENTATION;

    assert_eq!(
        presentation.machine.unwrap().metadata,
        GeneratedMachineMeta::TypedPresentation
    );
    assert_eq!(
        presentation
            .state(typed_presented_flow::StateId::QueuedTyped)
            .unwrap()
            .metadata,
        GeneratedStateMeta::Queued
    );
    assert_eq!(
        presentation.transition(event.transition).unwrap().metadata,
        GeneratedTransitionMeta::StartReview
    );
    assert_eq!(
        event.transition_in(graph).unwrap().method_name,
        "start_review_typed"
    );
    assert_eq!(
        graph.legal_targets(event.transition).unwrap(),
        &[typed_presented_flow::StateId::ReviewingTyped]
    );
}
