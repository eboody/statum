/// Static introspection surface emitted for a generated Statum machine.
pub trait MachineIntrospection {
    /// Machine-scoped state identifier emitted by `#[machine]`.
    type StateId: Copy + Eq + core::hash::Hash + 'static;

    /// Machine-scoped transition-site identifier emitted by `#[machine]`.
    type TransitionId: Copy + Eq + core::hash::Hash + 'static;

    /// Static graph descriptor for the machine family.
    const GRAPH: &'static MachineGraph<Self::StateId, Self::TransitionId>;
}

/// Runtime accessor for transition descriptors that may be supplied by a
/// distributed registration surface.
#[derive(Clone, Copy)]
pub struct TransitionInventory<S: 'static, T: 'static> {
    get: fn() -> &'static [TransitionDescriptor<S, T>],
}

impl<S, T> TransitionInventory<S, T> {
    /// Creates a transition inventory from a `'static` getter.
    pub const fn new(get: fn() -> &'static [TransitionDescriptor<S, T>]) -> Self {
        Self { get }
    }

    /// Returns the transition descriptors as a slice.
    pub fn as_slice(&self) -> &'static [TransitionDescriptor<S, T>] {
        (self.get)()
    }
}

impl<S, T> core::ops::Deref for TransitionInventory<S, T> {
    type Target = [TransitionDescriptor<S, T>];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<S, T> core::fmt::Debug for TransitionInventory<S, T> {
    fn fmt(
        &self,
        formatter: &mut core::fmt::Formatter<'_>,
    ) -> core::result::Result<(), core::fmt::Error> {
        formatter.debug_tuple("TransitionInventory").finish()
    }
}

impl<S, T> core::cmp::PartialEq for TransitionInventory<S, T> {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self.as_slice(), other.as_slice())
    }
}

impl<S, T> core::cmp::Eq for TransitionInventory<S, T> {}

/// Runtime accessor for transition presentation metadata that may be supplied
/// by a distributed registration surface.
#[derive(Clone, Copy)]
pub struct TransitionPresentationInventory<T: 'static, M: 'static = ()> {
    get: fn() -> &'static [TransitionPresentation<T, M>],
}

impl<T, M> TransitionPresentationInventory<T, M> {
    /// Creates a transition presentation inventory from a `'static` getter.
    pub const fn new(get: fn() -> &'static [TransitionPresentation<T, M>]) -> Self {
        Self { get }
    }

    /// Returns the transition presentation descriptors as a slice.
    pub fn as_slice(&self) -> &'static [TransitionPresentation<T, M>] {
        (self.get)()
    }
}

impl<T, M> core::ops::Deref for TransitionPresentationInventory<T, M> {
    type Target = [TransitionPresentation<T, M>];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T, M> core::fmt::Debug for TransitionPresentationInventory<T, M> {
    fn fmt(
        &self,
        formatter: &mut core::fmt::Formatter<'_>,
    ) -> core::result::Result<(), core::fmt::Error> {
        formatter
            .debug_tuple("TransitionPresentationInventory")
            .finish()
    }
}

impl<T, M> core::cmp::PartialEq for TransitionPresentationInventory<T, M> {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self.as_slice(), other.as_slice())
    }
}

impl<T, M> core::cmp::Eq for TransitionPresentationInventory<T, M> {}

/// Identity for one concrete machine state.
pub trait MachineStateIdentity: MachineIntrospection {
    /// The state id for this concrete machine instantiation.
    const STATE_ID: Self::StateId;
}

/// Optional human-facing metadata layered on top of a machine graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachinePresentation<
    S: 'static,
    T: 'static,
    MachineMeta: 'static = (),
    StateMeta: 'static = (),
    TransitionMeta: 'static = (),
> {
    /// Optional machine-level presentation metadata.
    pub machine: Option<MachinePresentationDescriptor<MachineMeta>>,
    /// Optional state-level presentation metadata keyed by state id.
    pub states: &'static [StatePresentation<S, StateMeta>],
    /// Optional transition-level presentation metadata keyed by transition id.
    pub transitions: TransitionPresentationInventory<T, TransitionMeta>,
}

impl<S, T, MachineMeta, StateMeta, TransitionMeta>
    MachinePresentation<S, T, MachineMeta, StateMeta, TransitionMeta>
where
    S: Copy + Eq + 'static,
    T: Copy + Eq + 'static,
{
    /// Finds state presentation metadata by id.
    pub fn state(&self, id: S) -> Option<&StatePresentation<S, StateMeta>> {
        self.states.iter().find(|state| state.id == id)
    }

    /// Finds transition presentation metadata by id.
    pub fn transition(&self, id: T) -> Option<&TransitionPresentation<T, TransitionMeta>> {
        self.transitions
            .iter()
            .find(|transition| transition.id == id)
    }
}

/// Optional machine-level presentation metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachinePresentationDescriptor<M: 'static = ()> {
    /// Optional short human-facing machine label.
    pub label: Option<&'static str>,
    /// Optional longer human-facing machine description.
    pub description: Option<&'static str>,
    /// Consumer-owned typed machine metadata.
    pub metadata: M,
}

/// Optional state-level presentation metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StatePresentation<S: 'static, M: 'static = ()> {
    /// Typed state identifier.
    pub id: S,
    /// Optional short human-facing state label.
    pub label: Option<&'static str>,
    /// Optional longer human-facing state description.
    pub description: Option<&'static str>,
    /// Consumer-owned typed state metadata.
    pub metadata: M,
}

/// Optional transition-level presentation metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionPresentation<T: 'static, M: 'static = ()> {
    /// Typed transition-site identifier.
    pub id: T,
    /// Optional short human-facing transition label.
    pub label: Option<&'static str>,
    /// Optional longer human-facing transition description.
    pub description: Option<&'static str>,
    /// Consumer-owned typed transition metadata.
    pub metadata: M,
}

/// A runtime record of one chosen transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordedTransition<S: 'static, T: 'static> {
    /// Rust-facing identity of the machine family.
    pub machine: MachineDescriptor,
    /// Exact source state where the transition was taken.
    pub from: S,
    /// Exact transition site that was chosen.
    pub transition: T,
    /// Exact target state that actually happened at runtime.
    pub chosen: S,
}

impl<S, T> RecordedTransition<S, T>
where
    S: 'static,
    T: 'static,
{
    /// Builds a runtime transition record from typed machine ids.
    pub const fn new(machine: MachineDescriptor, from: S, transition: T, chosen: S) -> Self {
        Self {
            machine,
            from,
            transition,
            chosen,
        }
    }

    /// Finds the static transition descriptor for this runtime event.
    pub fn transition_in<'a>(
        &self,
        graph: &'a MachineGraph<S, T>,
    ) -> Option<&'a TransitionDescriptor<S, T>>
    where
        S: Copy + Eq,
        T: Copy + Eq,
    {
        let descriptor = graph.transition(self.transition)?;
        if descriptor.from == self.from && descriptor.to.contains(&self.chosen) {
            Some(descriptor)
        } else {
            None
        }
    }

    /// Finds the static source-state descriptor for this runtime event.
    pub fn source_state_in<'a>(
        &self,
        graph: &'a MachineGraph<S, T>,
    ) -> Option<&'a StateDescriptor<S>>
    where
        S: Copy + Eq,
        T: Copy + Eq,
    {
        self.transition_in(graph)?;
        graph.state(self.from)
    }

    /// Finds the static chosen-target descriptor for this runtime event.
    pub fn chosen_state_in<'a>(
        &self,
        graph: &'a MachineGraph<S, T>,
    ) -> Option<&'a StateDescriptor<S>>
    where
        S: Copy + Eq,
        T: Copy + Eq,
    {
        self.transition_in(graph)?;
        graph.state(self.chosen)
    }
}

/// Runtime recording helpers layered on top of static machine introspection.
pub trait MachineTransitionRecorder: MachineStateIdentity {
    /// Records a runtime transition if `transition` is valid from `Self::STATE_ID`
    /// and `chosen` is one of its legal target states.
    fn try_record_transition(
        transition: Self::TransitionId,
        chosen: Self::StateId,
    ) -> Option<RecordedTransition<Self::StateId, Self::TransitionId>> {
        let graph = Self::GRAPH;
        let descriptor = graph.transition(transition)?;
        if descriptor.from != Self::STATE_ID || !descriptor.to.contains(&chosen) {
            return None;
        }

        Some(RecordedTransition::new(
            graph.machine,
            Self::STATE_ID,
            transition,
            chosen,
        ))
    }

    /// Records a runtime transition using a typed target machine state.
    fn try_record_transition_to<Next>(
        transition: Self::TransitionId,
    ) -> Option<RecordedTransition<Self::StateId, Self::TransitionId>>
    where
        Next: MachineStateIdentity<StateId = Self::StateId, TransitionId = Self::TransitionId>,
    {
        Self::try_record_transition(transition, Next::STATE_ID)
    }
}

impl<M> MachineTransitionRecorder for M where M: MachineStateIdentity {}

/// Structural machine graph emitted from macro-generated metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineGraph<S: 'static, T: 'static> {
    /// Rust-facing identity of the machine family.
    pub machine: MachineDescriptor,
    /// All states known to the machine.
    pub states: &'static [StateDescriptor<S>],
    /// All transition sites known to the machine.
    pub transitions: TransitionInventory<S, T>,
}

impl<S, T> MachineGraph<S, T>
where
    S: Copy + Eq + 'static,
    T: Copy + Eq + 'static,
{
    /// Finds a state descriptor by id.
    pub fn state(&self, id: S) -> Option<&StateDescriptor<S>> {
        self.states.iter().find(|state| state.id == id)
    }

    /// Finds a transition descriptor by id.
    pub fn transition(&self, id: T) -> Option<&TransitionDescriptor<S, T>> {
        self.transitions
            .iter()
            .find(|transition| transition.id == id)
    }

    /// Yields all transition sites originating from `state`.
    pub fn transitions_from(
        &self,
        state: S,
    ) -> impl Iterator<Item = &TransitionDescriptor<S, T>> + '_ {
        self.transitions
            .iter()
            .filter(move |transition| transition.from == state)
    }

    /// Finds the transition site for `method_name` on `state`.
    pub fn transition_from_method(
        &self,
        state: S,
        method_name: &str,
    ) -> Option<&TransitionDescriptor<S, T>> {
        self.transitions
            .iter()
            .find(|transition| transition.from == state && transition.method_name == method_name)
    }

    /// Yields all transition sites that share the same method name.
    pub fn transitions_named<'a>(
        &'a self,
        method_name: &'a str,
    ) -> impl Iterator<Item = &'a TransitionDescriptor<S, T>> + 'a {
        self.transitions
            .iter()
            .filter(move |transition| transition.method_name == method_name)
    }

    /// Returns the exact legal target states for a transition site.
    pub fn legal_targets(&self, id: T) -> Option<&'static [S]> {
        self.transition(id).map(|transition| transition.to)
    }
}

/// Rust-facing identity for a machine family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineDescriptor {
    /// `module_path!()` for the source module that owns the machine.
    pub module_path: &'static str,
    /// Fully qualified Rust type path for the machine family.
    pub rust_type_path: &'static str,
}

/// Static descriptor for one generated state id.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateDescriptor<S: 'static> {
    /// Typed state identifier.
    pub id: S,
    /// Rust variant name of the state marker.
    pub rust_name: &'static str,
    /// Whether the state carries `state_data`.
    pub has_data: bool,
}

/// Static descriptor for one transition site.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionDescriptor<S: 'static, T: 'static> {
    /// Typed transition-site identifier.
    pub id: T,
    /// Rust method name for the transition site.
    pub method_name: &'static str,
    /// Exact source state for the transition site.
    pub from: S,
    /// Exact legal target states for the transition site.
    pub to: &'static [S],
}

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
