use super::{
    MachineDescriptor, MachineGraph, MachineStateIdentity, StateDescriptor, TransitionDescriptor,
};

/// Stable low-cardinality labels for one runtime transition event.
///
/// These labels are suitable for tracing spans and metric attributes because
/// they are derived from generated machine metadata, not from per-request or
/// user-provided values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionTelemetryLabels {
    /// Stable machine family label.
    pub machine: &'static str,
    /// Stable source-state label.
    pub from_state: &'static str,
    /// Stable transition-site label.
    pub transition: &'static str,
    /// Stable chosen target-state label.
    pub chosen_state: &'static str,
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
        if self.machine != graph.machine {
            return None;
        }

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

    /// Joins a runtime transition event back to stable telemetry labels.
    ///
    /// Returns `None` when the runtime event cannot be validated against `graph`.
    /// Successful labels are low-cardinality and come only from generated machine
    /// metadata: machine type path, source state name, transition method name,
    /// and chosen target state name.
    pub fn telemetry_labels_in(
        &self,
        graph: &MachineGraph<S, T>,
    ) -> Option<TransitionTelemetryLabels>
    where
        S: Copy + Eq,
        T: Copy + Eq,
    {
        let transition = self.transition_in(graph)?;
        let from_state = graph.state(self.from)?;
        let chosen_state = graph.state(self.chosen)?;

        Some(TransitionTelemetryLabels {
            machine: graph.machine_label(),
            from_state: from_state.rust_name,
            transition: transition.method_name,
            chosen_state: chosen_state.rust_name,
        })
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
