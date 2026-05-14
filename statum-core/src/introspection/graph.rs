use super::TransitionInventory;

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
