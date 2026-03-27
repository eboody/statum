//! Static graph export built directly from `statum::MachineIntrospection::GRAPH`.
//!
//! This crate is authoritative only for machine-local topology:
//! machine identity, states, transition sites, exact legal targets, and
//! graph roots derivable from the static graph itself.
//!
//! For linked-build codebase export, use [`codebase::CodebaseDoc`]. That
//! surface combines every linked compiled machine family, declared
//! validator-entry surfaces emitted by compiled `#[validators]` impls, direct
//! construction availability per state, legacy direct payload links, and exact
//! static relations inferred from supported type syntax plus nominal
//! `#[machine_ref(...)]` declarations. Validator node labels use the impl self
//! type as written in source, so they are display syntax rather than canonical
//! Rust type identity. Method-level `#[cfg]` and `#[cfg_attr]` on validator
//! methods are rejected at the macro layer. `include!()`-generated validator
//! impls are also rejected. The linked codebase surface also carries source
//! rustdoc separately as `docs` on machines, states, transitions, and
//! validator-entry surfaces.
//!
//! Use [`MachineDoc::from_machine`] for Statum-generated machine families and
//! [`MachineDoc::try_from_graph`] when you need to validate an externally
//! supplied [`MachineGraph`] before rendering or traversal.
//!
//! This crate does not model orchestration order across machines or
//! runtime-selected branches for one run. Optional presentation metadata may
//! be joined onto the validated machine graph for renderer output, but it does
//! not change the authoritative structural surface. Use
//! `#[present(description = ...)]` for concise renderer copy and ordinary outer
//! rustdoc comments (`///`) for fuller codebase/inspector detail.

use std::collections::{HashMap, HashSet};

use statum::{
    MachineDescriptor, MachineGraph, MachineIntrospection, StateDescriptor, TransitionDescriptor,
};

pub mod codebase;
mod export;
pub mod render;

pub use codebase::{
    CodebaseDoc, CodebaseDocError, CodebaseLink, CodebaseMachine, CodebaseMachineRelationGroup,
    CodebaseRelation, CodebaseRelationBasis, CodebaseRelationCount, CodebaseRelationDetail,
    CodebaseRelationKind, CodebaseRelationSource, CodebaseState, CodebaseTransition,
    CodebaseValidatorEntry,
};
pub use export::{
    ExportDoc, ExportDocError, ExportMachine, ExportSource, ExportState, ExportTransition,
};

/// Static machine graph exported directly from `MachineIntrospection::GRAPH`.
///
/// This type is authoritative only for machine-local topology:
/// states, transition sites, exact legal targets, and graph roots derivable
/// from the static graph itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineDoc<S: 'static, T: 'static> {
    machine: MachineDescriptor,
    states: Vec<StateDoc<S>>,
    edges: Vec<EdgeDoc<S, T>>,
}

/// Error returned when a `MachineGraph` cannot be exported into a `MachineDoc`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineDocError {
    /// The graph's state list is empty.
    EmptyStateList { machine: &'static str },
    /// One state id appears more than once in the graph's state list.
    DuplicateStateId {
        machine: &'static str,
        state: &'static str,
    },
    /// One transition id appears more than once in the graph's transition list.
    DuplicateTransitionId {
        machine: &'static str,
        transition: &'static str,
    },
    /// One source state declares the same transition method name more than once.
    DuplicateTransitionSite {
        machine: &'static str,
        state: &'static str,
        transition: &'static str,
    },
    /// One transition source state is not present in the graph's state list.
    MissingSourceState {
        machine: &'static str,
        transition: &'static str,
    },
    /// One transition target state is not present in the graph's state list.
    MissingTargetState {
        machine: &'static str,
        transition: &'static str,
    },
    /// One transition site declares no legal target states.
    EmptyTargetSet {
        machine: &'static str,
        transition: &'static str,
    },
    /// One transition lists the same target state more than once.
    DuplicateTargetState {
        machine: &'static str,
        transition: &'static str,
        state: &'static str,
    },
}

impl core::fmt::Display for MachineDocError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyStateList { machine } => write!(
                formatter,
                "machine graph `{machine}` contains no states"
            ),
            Self::DuplicateStateId { machine, state } => write!(
                formatter,
                "machine graph `{machine}` contains duplicate state id for state `{state}`"
            ),
            Self::DuplicateTransitionId {
                machine,
                transition,
            } => write!(
                formatter,
                "machine graph `{machine}` contains duplicate transition id for transition `{transition}`"
            ),
            Self::DuplicateTransitionSite {
                machine,
                state,
                transition,
            } => write!(
                formatter,
                "machine graph `{machine}` contains duplicate transition site `{state}::{transition}`"
            ),
            Self::MissingSourceState {
                machine,
                transition,
            } => write!(
                formatter,
                "machine graph `{machine}` contains transition `{transition}` whose source state is missing from the state list"
            ),
            Self::MissingTargetState {
                machine,
                transition,
            } => write!(
                formatter,
                "machine graph `{machine}` contains transition `{transition}` whose target state is missing from the state list"
            ),
            Self::EmptyTargetSet {
                machine,
                transition,
            } => write!(
                formatter,
                "machine graph `{machine}` contains transition `{transition}` with no target states"
            ),
            Self::DuplicateTargetState {
                machine,
                transition,
                state,
            } => write!(
                formatter,
                "machine graph `{machine}` contains transition `{transition}` with duplicate target state `{state}`"
            ),
        }
    }
}

impl std::error::Error for MachineDocError {}

impl<S, T> TryFrom<&'static MachineGraph<S, T>> for MachineDoc<S, T>
where
    S: Copy + Eq + std::hash::Hash + 'static,
    T: Copy + Eq + 'static,
{
    type Error = MachineDocError;

    fn try_from(graph: &'static MachineGraph<S, T>) -> Result<Self, Self::Error> {
        Self::try_from_graph(graph)
    }
}

impl<S, T> MachineDoc<S, T> {
    /// Descriptor for the exported machine family.
    pub fn machine(&self) -> MachineDescriptor {
        self.machine
    }

    /// Exported states in the same order as the underlying static graph.
    pub fn states(&self) -> &[StateDoc<S>] {
        &self.states
    }

    /// Exported transition sites sorted stably for deterministic renderers.
    pub fn edges(&self) -> &[EdgeDoc<S, T>] {
        &self.edges
    }
}

impl<S, T> MachineDoc<S, T>
where
    S: Copy + Eq + 'static,
{
    /// Returns the exported state descriptor for one generated state id.
    pub fn state(&self, id: S) -> Option<&StateDoc<S>> {
        self.states.iter().find(|state| state.descriptor.id == id)
    }
}

impl<S, T> MachineDoc<S, T> {
    /// Returns every state with no incoming edge in the exported topology.
    pub fn roots(&self) -> impl Iterator<Item = &StateDoc<S>> {
        self.states.iter().filter(|state| state.is_root)
    }
}

impl<S, T> MachineDoc<S, T>
where
    S: Copy + Eq + std::hash::Hash + 'static,
    T: Copy + Eq + 'static,
{
    /// Exports one machine family from a concrete `MachineIntrospection` type.
    ///
    /// This is the normal entry point when the graph comes from Statum itself.
    /// It will panic only if Statum emitted an invalid
    /// `MachineIntrospection::GRAPH`.
    pub fn from_machine<M>() -> Self
    where
        M: MachineIntrospection<StateId = S, TransitionId = T>,
    {
        Self::try_from_graph(M::GRAPH)
            .expect("Statum emitted an invalid MachineIntrospection::GRAPH")
    }

    /// Exports one externally supplied machine graph after validating it.
    ///
    /// Use this when the graph does not come from a concrete Statum machine
    /// type and you want malformed external graphs to fail closed with
    /// [`MachineDocError`] instead of being rendered best-effort.
    pub fn try_from_graph(graph: &'static MachineGraph<S, T>) -> Result<Self, MachineDocError> {
        let transitions = graph.transitions.as_slice();
        validate_graph(graph.machine, graph.states, transitions)?;
        let incoming = incoming_states(transitions);
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

        let mut edges = transitions
            .iter()
            .copied()
            .map(|descriptor| EdgeDoc { descriptor })
            .collect::<Vec<_>>();
        edges.sort_by(|left, right| compare_edges(&state_positions, left, right));

        Ok(Self {
            machine: graph.machine,
            states,
            edges,
        })
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

fn validate_graph<S, T>(
    machine: MachineDescriptor,
    states: &[StateDescriptor<S>],
    transitions: &[TransitionDescriptor<S, T>],
) -> Result<(), MachineDocError>
where
    S: Copy + Eq + std::hash::Hash + 'static,
    T: Copy + Eq + 'static,
{
    if states.is_empty() {
        return Err(MachineDocError::EmptyStateList {
            machine: machine.rust_type_path,
        });
    }

    let mut state_names = HashMap::with_capacity(states.len());
    for state in states.iter() {
        if state_names.insert(state.id, state.rust_name).is_some() {
            return Err(MachineDocError::DuplicateStateId {
                machine: machine.rust_type_path,
                state: state.rust_name,
            });
        }
    }

    let mut transition_sites = HashSet::with_capacity(transitions.len());
    let mut transition_ids = Vec::with_capacity(transitions.len());
    for transition in transitions.iter() {
        if transition_ids.contains(&transition.id) {
            return Err(MachineDocError::DuplicateTransitionId {
                machine: machine.rust_type_path,
                transition: transition.method_name,
            });
        }
        transition_ids.push(transition.id);

        if !state_names.contains_key(&transition.from) {
            return Err(MachineDocError::MissingSourceState {
                machine: machine.rust_type_path,
                transition: transition.method_name,
            });
        }

        let from_state_name = state_names[&transition.from];
        if !transition_sites.insert((transition.from, transition.method_name)) {
            return Err(MachineDocError::DuplicateTransitionSite {
                machine: machine.rust_type_path,
                state: from_state_name,
                transition: transition.method_name,
            });
        }

        if transition.to.is_empty() {
            return Err(MachineDocError::EmptyTargetSet {
                machine: machine.rust_type_path,
                transition: transition.method_name,
            });
        }

        let mut seen_targets = HashSet::with_capacity(transition.to.len());
        for target in transition.to.iter().copied() {
            let Some(state_name) = state_names.get(&target).copied() else {
                return Err(MachineDocError::MissingTargetState {
                    machine: machine.rust_type_path,
                    transition: transition.method_name,
                });
            };

            if !seen_targets.insert(target) {
                return Err(MachineDocError::DuplicateTargetState {
                    machine: machine.rust_type_path,
                    transition: transition.method_name,
                    state: state_name,
                });
            }
        }
    }

    Ok(())
}

fn incoming_states<S, T>(transitions: &[TransitionDescriptor<S, T>]) -> HashSet<S>
where
    S: Copy + Eq + std::hash::Hash + 'static,
    T: Copy + Eq + 'static,
{
    let mut incoming = HashSet::new();
    for transition in transitions.iter() {
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
