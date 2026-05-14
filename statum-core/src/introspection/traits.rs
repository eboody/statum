use super::MachineGraph;

/// Static introspection surface emitted for a generated Statum machine.
pub trait MachineIntrospection {
    /// Machine-scoped state identifier emitted by `#[machine]`.
    type StateId: Copy + Eq + core::hash::Hash + 'static;

    /// Machine-scoped transition-site identifier emitted by `#[machine]`.
    type TransitionId: Copy + Eq + core::hash::Hash + 'static;

    /// Static graph descriptor for the machine family.
    const GRAPH: &'static MachineGraph<Self::StateId, Self::TransitionId>;
}

/// Identity for one concrete machine state.
pub trait MachineStateIdentity: MachineIntrospection {
    /// The state id for this concrete machine instantiation.
    const STATE_ID: Self::StateId;
}
