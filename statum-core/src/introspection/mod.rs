mod graph;
mod inventory;
mod presentation;
mod recording;
mod traits;

pub use graph::{MachineDescriptor, MachineGraph, StateDescriptor, TransitionDescriptor};
pub use inventory::{TransitionInventory, TransitionPresentationInventory};
pub use presentation::{
    MachinePresentation, MachinePresentationDescriptor, StatePresentation, TransitionPresentation,
};
pub use recording::{MachineTransitionRecorder, RecordedTransition};
pub use traits::{MachineIntrospection, MachineStateIdentity};

#[cfg(test)]
mod tests {
    use super::{
        MachineDescriptor, MachineGraph, MachineIntrospection, MachinePresentation,
        MachinePresentationDescriptor, MachineStateIdentity, MachineTransitionRecorder,
        RecordedTransition, StateDescriptor, StatePresentation, TransitionDescriptor,
        TransitionInventory, TransitionPresentation, TransitionPresentationInventory,
    };
    use core::marker::PhantomData;

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
    enum StateId {
        Draft,
        Review,
        Published,
    }

    #[derive(Clone, Copy)]
    struct TransitionId(&'static crate::__private::TransitionToken);

    impl TransitionId {
        const fn from_token(token: &'static crate::__private::TransitionToken) -> Self {
            Self(token)
        }
    }

    impl core::fmt::Debug for TransitionId {
        fn fmt(
            &self,
            formatter: &mut core::fmt::Formatter<'_>,
        ) -> core::result::Result<(), core::fmt::Error> {
            formatter.write_str("TransitionId(..)")
        }
    }

    impl core::cmp::PartialEq for TransitionId {
        fn eq(&self, other: &Self) -> bool {
            core::ptr::eq(self.0, other.0)
        }
    }

    impl core::cmp::Eq for TransitionId {}

    impl core::hash::Hash for TransitionId {
        fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
            let ptr = core::ptr::from_ref(self.0) as usize;
            <usize as core::hash::Hash>::hash(&ptr, state);
        }
    }

    static REVIEW_TARGETS: [StateId; 1] = [StateId::Review];
    static PUBLISH_TARGETS: [StateId; 1] = [StateId::Published];
    static SUBMIT_FROM_DRAFT_TOKEN: crate::__private::TransitionToken =
        crate::__private::TransitionToken::new();
    static PUBLISH_FROM_REVIEW_TOKEN: crate::__private::TransitionToken =
        crate::__private::TransitionToken::new();
    const SUBMIT_FROM_DRAFT: TransitionId = TransitionId::from_token(&SUBMIT_FROM_DRAFT_TOKEN);
    const PUBLISH_FROM_REVIEW: TransitionId = TransitionId::from_token(&PUBLISH_FROM_REVIEW_TOKEN);
    static STATES: [StateDescriptor<StateId>; 3] = [
        StateDescriptor {
            id: StateId::Draft,
            rust_name: "Draft",
            has_data: false,
        },
        StateDescriptor {
            id: StateId::Review,
            rust_name: "Review",
            has_data: true,
        },
        StateDescriptor {
            id: StateId::Published,
            rust_name: "Published",
            has_data: false,
        },
    ];
    static TRANSITIONS: [TransitionDescriptor<StateId, TransitionId>; 2] = [
        TransitionDescriptor {
            id: SUBMIT_FROM_DRAFT,
            method_name: "submit",
            from: StateId::Draft,
            to: &REVIEW_TARGETS,
        },
        TransitionDescriptor {
            id: PUBLISH_FROM_REVIEW,
            method_name: "publish",
            from: StateId::Review,
            to: &PUBLISH_TARGETS,
        },
    ];
    static TRANSITION_PRESENTATIONS: [TransitionPresentation<TransitionId, TransitionMeta>; 2] = [
        TransitionPresentation {
            id: SUBMIT_FROM_DRAFT,
            label: Some("Submit"),
            description: Some("Move work into review."),
            metadata: TransitionMeta {
                phase: Phase::Review,
                branch: false,
            },
        },
        TransitionPresentation {
            id: PUBLISH_FROM_REVIEW,
            label: Some("Publish"),
            description: Some("Complete the workflow."),
            metadata: TransitionMeta {
                phase: Phase::Output,
                branch: false,
            },
        },
    ];

    struct Workflow<S>(PhantomData<S>);
    struct DraftMarker;
    struct ReviewMarker;
    struct PublishedMarker;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Phase {
        Intake,
        Review,
        Output,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct MachineMeta {
        phase: Phase,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct StateMeta {
        phase: Phase,
        term: &'static str,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct TransitionMeta {
        phase: Phase,
        branch: bool,
    }

    static PRESENTATION: MachinePresentation<
        StateId,
        TransitionId,
        MachineMeta,
        StateMeta,
        TransitionMeta,
    > = MachinePresentation {
        machine: Some(MachinePresentationDescriptor {
            label: Some("Workflow"),
            description: Some("Example presentation metadata for introspection."),
            metadata: MachineMeta {
                phase: Phase::Intake,
            },
        }),
        states: &[
            StatePresentation {
                id: StateId::Draft,
                label: Some("Draft"),
                description: Some("Work has not been submitted yet."),
                metadata: StateMeta {
                    phase: Phase::Intake,
                    term: "draft",
                },
            },
            StatePresentation {
                id: StateId::Review,
                label: Some("Review"),
                description: Some("Work is awaiting review."),
                metadata: StateMeta {
                    phase: Phase::Review,
                    term: "review",
                },
            },
            StatePresentation {
                id: StateId::Published,
                label: Some("Published"),
                description: Some("Work is complete."),
                metadata: StateMeta {
                    phase: Phase::Output,
                    term: "published",
                },
            },
        ],
        transitions: TransitionPresentationInventory::new(|| &TRANSITION_PRESENTATIONS),
    };

    impl<S> MachineIntrospection for Workflow<S> {
        type StateId = StateId;
        type TransitionId = TransitionId;

        const GRAPH: &'static MachineGraph<Self::StateId, Self::TransitionId> = &MachineGraph {
            machine: MachineDescriptor {
                module_path: "workflow",
                rust_type_path: "workflow::Machine",
            },
            states: &STATES,
            transitions: TransitionInventory::new(|| &TRANSITIONS),
        };
    }

    impl MachineStateIdentity for Workflow<DraftMarker> {
        const STATE_ID: Self::StateId = StateId::Draft;
    }

    impl MachineStateIdentity for Workflow<ReviewMarker> {
        const STATE_ID: Self::StateId = StateId::Review;
    }

    impl MachineStateIdentity for Workflow<PublishedMarker> {
        const STATE_ID: Self::StateId = StateId::Published;
    }

    #[test]
    fn query_helpers_find_expected_items() {
        let graph = MachineGraph {
            machine: MachineDescriptor {
                module_path: "workflow",
                rust_type_path: "workflow::Machine",
            },
            states: &STATES,
            transitions: TransitionInventory::new(|| &TRANSITIONS),
        };

        assert_eq!(
            graph.state(StateId::Review).map(|state| state.rust_name),
            Some("Review")
        );
        assert_eq!(
            graph
                .transition(PUBLISH_FROM_REVIEW)
                .map(|transition| transition.method_name),
            Some("publish")
        );
        assert_eq!(
            graph
                .transition_from_method(StateId::Draft, "submit")
                .map(|transition| transition.id),
            Some(SUBMIT_FROM_DRAFT)
        );
        assert_eq!(
            graph.legal_targets(SUBMIT_FROM_DRAFT),
            Some(REVIEW_TARGETS.as_slice())
        );
        assert_eq!(graph.transitions_from(StateId::Draft).count(), 1);
        assert_eq!(graph.transitions_named("publish").count(), 1);
    }

    #[test]
    fn runtime_transition_recording_joins_back_to_static_graph() {
        let event = Workflow::<DraftMarker>::try_record_transition_to::<Workflow<ReviewMarker>>(
            SUBMIT_FROM_DRAFT,
        )
        .expect("valid runtime transition");

        assert_eq!(
            event,
            RecordedTransition::new(
                MachineDescriptor {
                    module_path: "workflow",
                    rust_type_path: "workflow::Machine",
                },
                StateId::Draft,
                SUBMIT_FROM_DRAFT,
                StateId::Review,
            )
        );
        assert_eq!(
            Workflow::<DraftMarker>::GRAPH
                .transition(event.transition)
                .map(|transition| (transition.from, transition.to)),
            Some((StateId::Draft, REVIEW_TARGETS.as_slice()))
        );
        assert_eq!(
            event.source_state_in(Workflow::<DraftMarker>::GRAPH),
            Some(&StateDescriptor {
                id: StateId::Draft,
                rust_name: "Draft",
                has_data: false,
            })
        );
    }

    #[test]
    fn runtime_transition_recording_rejects_illegal_target_or_site() {
        assert!(Workflow::<DraftMarker>::try_record_transition(
            PUBLISH_FROM_REVIEW,
            StateId::Published,
        )
        .is_none());
        assert!(
            Workflow::<ReviewMarker>::try_record_transition_to::<Workflow<PublishedMarker>>(
                SUBMIT_FROM_DRAFT,
            )
            .is_none()
        );
    }

    #[test]
    fn runtime_transition_recording_rejects_matching_transition_from_different_machine() {
        let event = RecordedTransition::new(
            MachineDescriptor {
                module_path: "other_workflow",
                rust_type_path: "other_workflow::Machine",
            },
            StateId::Draft,
            SUBMIT_FROM_DRAFT,
            StateId::Review,
        );

        assert_eq!(event.transition_in(Workflow::<DraftMarker>::GRAPH), None);
        assert_eq!(event.source_state_in(Workflow::<DraftMarker>::GRAPH), None);
        assert_eq!(event.chosen_state_in(Workflow::<DraftMarker>::GRAPH), None);
    }

    #[test]
    fn presentation_queries_join_with_runtime_transitions() {
        let event = Workflow::<DraftMarker>::try_record_transition_to::<Workflow<ReviewMarker>>(
            SUBMIT_FROM_DRAFT,
        )
        .expect("valid runtime transition");

        assert_eq!(
            PRESENTATION.machine,
            Some(MachinePresentationDescriptor {
                label: Some("Workflow"),
                description: Some("Example presentation metadata for introspection."),
                metadata: MachineMeta {
                    phase: Phase::Intake,
                },
            })
        );
        assert_eq!(
            PRESENTATION.transition(event.transition),
            Some(&TransitionPresentation {
                id: SUBMIT_FROM_DRAFT,
                label: Some("Submit"),
                description: Some("Move work into review."),
                metadata: TransitionMeta {
                    phase: Phase::Review,
                    branch: false,
                },
            })
        );
        assert_eq!(
            PRESENTATION.state(event.chosen),
            Some(&StatePresentation {
                id: StateId::Review,
                label: Some("Review"),
                description: Some("Work is awaiting review."),
                metadata: StateMeta {
                    phase: Phase::Review,
                    term: "review",
                },
            })
        );
    }
}
