use super::TransitionPresentationInventory;

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
