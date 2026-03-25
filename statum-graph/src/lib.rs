//! Static graph export built directly from `statum::MachineIntrospection::GRAPH`.
//!
//! This crate is authoritative only for machine-local topology:
//! machine identity, states, transition sites, exact legal targets, and
//! roots derivable from the static graph itself.
//!
//! It does not model orchestration order across machines, runtime-selected
//! branches for one run, or any consumer-owned presentation metadata.

use std::collections::{HashMap, HashSet};

use statum::{
    MachineDescriptor, MachineGraph, MachineIntrospection, StateDescriptor, TransitionDescriptor,
};

pub mod render;

/// Static machine graph exported directly from `MachineIntrospection::GRAPH`.
///
/// This type is authoritative only for machine-local topology:
/// states, transition sites, exact legal targets, and roots derivable
/// from the static graph itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineDoc<S: 'static, T: 'static> {
    /// Descriptor for the exported machine family.
    pub machine: MachineDescriptor,
    /// Exported states in the same order as the underlying static graph.
    pub states: Vec<StateDoc<S>>,
    /// Exported transition sites sorted stably for deterministic renderers.
    pub edges: Vec<EdgeDoc<S, T>>,
}

impl<S, T> MachineDoc<S, T>
where
    S: Copy + Eq + std::hash::Hash + 'static,
    T: Copy + Eq + 'static,
{
    /// Exports one machine family from a concrete `MachineIntrospection` type.
    pub fn from_machine<M>() -> Self
    where
        M: MachineIntrospection<StateId = S, TransitionId = T>,
    {
        Self::from_graph(M::GRAPH)
    }

    /// Exports one machine graph from a static `MachineGraph`.
    pub fn from_graph(graph: &'static MachineGraph<S, T>) -> Self {
        let incoming = incoming_states(graph);
        let state_positions = state_positions(graph.states);

        let states = graph
            .states
            .iter()
            .copied()
            .map(|descriptor| StateDoc {
                descriptor,
                is_root: !incoming.contains(&descriptor.id),
            })
            .collect();

        let mut edges = graph
            .transitions
            .iter()
            .copied()
            .map(|descriptor| EdgeDoc { descriptor })
            .collect::<Vec<_>>();
        edges.sort_by(|left, right| compare_edges(&state_positions, left, right));

        Self {
            machine: graph.machine,
            states,
            edges,
        }
    }

    /// Returns the exported state descriptor for one generated state id.
    pub fn state(&self, id: S) -> Option<&StateDoc<S>> {
        self.states.iter().find(|state| state.descriptor.id == id)
    }

    /// Returns every state with no incoming edge in the exported topology.
    pub fn roots(&self) -> impl Iterator<Item = &StateDoc<S>> {
        self.states.iter().filter(|state| state.is_root)
    }
}

/// Exported state metadata for one graph node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateDoc<S: 'static> {
    /// Underlying descriptor from `statum`.
    pub descriptor: StateDescriptor<S>,
    /// True when the exported topology has no incoming edge for this state.
    pub is_root: bool,
}

/// Exported transition metadata for one graph edge site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeDoc<S: 'static, T: 'static> {
    /// Underlying descriptor from `statum`.
    pub descriptor: TransitionDescriptor<S, T>,
}

fn incoming_states<S, T>(graph: &MachineGraph<S, T>) -> HashSet<S>
where
    S: Copy + Eq + std::hash::Hash + 'static,
    T: Copy + Eq + 'static,
{
    let mut incoming = HashSet::new();
    for transition in graph.transitions.iter() {
        for target in transition.to.iter().copied() {
            incoming.insert(target);
        }
    }

    incoming
}

fn state_positions<S>(states: &[StateDescriptor<S>]) -> HashMap<S, usize>
where
    S: Copy + Eq + std::hash::Hash + 'static,
{
    states
        .iter()
        .enumerate()
        .map(|(index, state)| (state.id, index))
        .collect()
}

fn compare_edges<S, T>(
    state_positions: &HashMap<S, usize>,
    left: &EdgeDoc<S, T>,
    right: &EdgeDoc<S, T>,
) -> std::cmp::Ordering
where
    S: Copy + Eq + std::hash::Hash + 'static,
    T: Copy + Eq + 'static,
{
    state_positions[&left.descriptor.from]
        .cmp(&state_positions[&right.descriptor.from])
        .then_with(|| {
            left.descriptor
                .method_name
                .cmp(right.descriptor.method_name)
        })
        .then_with(|| compare_targets(state_positions, left.descriptor.to, right.descriptor.to))
}

fn compare_targets<S>(
    state_positions: &HashMap<S, usize>,
    left: &[S],
    right: &[S],
) -> std::cmp::Ordering
where
    S: Copy + Eq + std::hash::Hash + 'static,
{
    let left = left.iter().map(|state| state_positions[state]);
    let right = right.iter().map(|state| state_positions[state]);

    left.cmp(right)
}
