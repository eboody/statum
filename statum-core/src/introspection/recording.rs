use super::{
    MachineDescriptor, MachineGraph, MachineStateIdentity, StateDescriptor, TransitionDescriptor,
};

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
