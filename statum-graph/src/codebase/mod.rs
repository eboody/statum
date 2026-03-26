use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use serde::Serialize;
use statum::{
    LinkedMachineGraph, LinkedReferenceTypeDescriptor, LinkedRelationBasis,
    LinkedRelationDescriptor, LinkedRelationKind, LinkedRelationSource, LinkedRelationTarget,
    LinkedValidatorEntryDescriptor, StaticMachineLinkDescriptor,
};

pub mod render;

/// Stable export model for the linked compiled machine inventory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CodebaseDoc {
    machines: Vec<CodebaseMachine>,
    links: Vec<CodebaseLink>,
    relations: Vec<CodebaseRelation>,
}

impl CodebaseDoc {
    /// Builds a combined codebase document from every linked machine visible to
    /// the current build.
    pub fn linked() -> Result<Self, CodebaseDocError> {
        Self::try_from_linked_with_inventories(
            statum::linked_machines(),
            statum::linked_validator_entries(),
            statum::linked_relations(),
            statum::linked_reference_types(),
        )
    }

    /// Builds a combined codebase document from an explicit linked machine
    /// inventory.
    pub fn try_from_linked(
        linked: &'static [LinkedMachineGraph],
    ) -> Result<Self, CodebaseDocError> {
        Self::try_from_linked_with_inventories(linked, &[], &[], &[])
    }

    /// Builds a combined codebase document from explicit linked machine and
    /// validator-entry inventories.
    pub fn try_from_linked_with_validator_entries(
        linked: &'static [LinkedMachineGraph],
        validator_entries: &'static [LinkedValidatorEntryDescriptor],
    ) -> Result<Self, CodebaseDocError> {
        Self::try_from_linked_with_inventories(linked, validator_entries, &[], &[])
    }

    fn try_from_linked_with_inventories(
        linked: &'static [LinkedMachineGraph],
        validator_entries: &'static [LinkedValidatorEntryDescriptor],
        relations: &'static [LinkedRelationDescriptor],
        reference_types: &'static [LinkedReferenceTypeDescriptor],
    ) -> Result<Self, CodebaseDocError> {
        let mut linked = linked.to_vec();
        linked.sort_by(|left, right| {
            left.machine
                .rust_type_path
                .cmp(right.machine.rust_type_path)
        });

        let mut machines = Vec::with_capacity(linked.len());
        let mut machine_paths = HashSet::with_capacity(linked.len());
        let mut static_links = Vec::with_capacity(linked.len());

        for (machine_index, machine) in linked.iter().enumerate() {
            if !machine_paths.insert(machine.machine.rust_type_path) {
                return Err(CodebaseDocError::DuplicateMachine {
                    machine: machine.machine.rust_type_path,
                });
            }

            let (built_machine, built_links) = build_machine(machine_index, *machine)?;
            machines.push(built_machine);
            static_links.push(built_links);
        }

        let resolved_validator_entries =
            resolve_validator_entries(&mut machines, validator_entries)?;
        let links = resolve_static_links(&machines, &static_links)?;
        let relations = resolve_relations(&machines, relations, reference_types)?;

        debug_assert_eq!(
            resolved_validator_entries,
            total_validator_entries(&machines)
        );

        Ok(Self {
            machines,
            links,
            relations,
        })
    }

    /// Exported machines in stable codebase order.
    pub fn machines(&self) -> &[CodebaseMachine] {
        &self.machines
    }

    /// Resolved static cross-machine links in stable order.
    pub fn links(&self) -> &[CodebaseLink] {
        &self.links
    }

    /// Resolved exact cross-machine relations in stable order.
    pub fn relations(&self) -> &[CodebaseRelation] {
        &self.relations
    }

    /// Returns one exported machine by its stable codebase index.
    pub fn machine(&self, index: usize) -> Option<&CodebaseMachine> {
        self.machines.get(index)
    }
}

/// One machine family in the codebase export surface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CodebaseMachine {
    /// Stable codebase-local machine index.
    pub index: usize,
    /// `module_path!()` for the source module that owns the machine.
    pub module_path: &'static str,
    /// Fully qualified Rust type path for the machine family.
    pub rust_type_path: &'static str,
    /// Optional human-facing machine label.
    pub label: Option<&'static str>,
    /// Optional human-facing machine description.
    pub description: Option<&'static str>,
    /// States exported in source order.
    pub states: Vec<CodebaseState>,
    /// Transition sites exported in deterministic order.
    pub transitions: Vec<CodebaseTransition>,
    /// Declared validator-entry surfaces exported in deterministic order.
    pub validator_entries: Vec<CodebaseValidatorEntry>,
}

impl CodebaseMachine {
    /// Returns one exported state by its stable state index.
    pub fn state(&self, index: usize) -> Option<&CodebaseState> {
        self.states.get(index)
    }

    /// Returns one exported state by its Rust state name.
    pub fn state_named(&self, rust_name: &str) -> Option<&CodebaseState> {
        self.states
            .iter()
            .find(|state| state.rust_name == rust_name)
    }

    /// Returns one exported validator-entry surface by its stable machine-local
    /// index.
    pub fn validator_entry(&self, index: usize) -> Option<&CodebaseValidatorEntry> {
        self.validator_entries.get(index)
    }

    /// Stable renderer node id for one state in this machine.
    pub fn node_id(&self, state_index: usize) -> String {
        format!("m{}_s{}", self.index, state_index)
    }

    /// Stable renderer node id for one validator entry in this machine.
    pub fn validator_node_id(&self, entry_index: usize) -> String {
        format!("m{}_v{}", self.index, entry_index)
    }

    fn cluster_id(&self) -> String {
        format!("m{}", self.index)
    }

    fn display_label(&self) -> Cow<'static, str> {
        match self.label {
            Some(label) => Cow::Borrowed(label),
            None => Cow::Borrowed(self.rust_type_path),
        }
    }

    fn transition_site(&self, state: &str, method_name: &str) -> Option<&CodebaseTransition> {
        self.transitions.iter().find(|transition| {
            transition.method_name == method_name
                && self
                    .state(transition.from)
                    .is_some_and(|source| source.rust_name == state)
        })
    }
}

/// One state in the codebase export surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CodebaseState {
    /// Stable machine-local state index.
    pub index: usize,
    /// Rust variant name emitted by Statum.
    pub rust_name: &'static str,
    /// Optional human-facing state label.
    pub label: Option<&'static str>,
    /// Optional human-facing state description.
    pub description: Option<&'static str>,
    /// Whether the state carries `state_data`.
    pub has_data: bool,
    /// Whether direct construction is available for this state.
    pub direct_construction_available: bool,
    /// Whether the state has no incoming transition in its machine.
    pub is_graph_root: bool,
}

impl CodebaseState {
    /// Human-facing state label used by text renderers.
    pub fn display_label(&self) -> Cow<'static, str> {
        match self.label {
            Some(label) => Cow::Borrowed(label),
            None if self.has_data => Cow::Owned(format!("{} (data)", self.rust_name)),
            None => Cow::Borrowed(self.rust_name),
        }
    }
}

/// One declared validator-entry surface in the codebase export surface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CodebaseValidatorEntry {
    /// Stable machine-local validator-entry index.
    pub index: usize,
    /// `module_path!()` for the module that owns the `#[validators]` impl.
    pub source_module_path: &'static str,
    /// Human-facing source syntax for the persisted impl self type as written.
    pub source_type_display: &'static str,
    /// Stable target-state indices in machine state order.
    pub target_states: Vec<usize>,
}

impl CodebaseValidatorEntry {
    /// Human-facing node label used by text renderers.
    pub fn display_label(&self) -> Cow<'static, str> {
        Cow::Owned(format!("{}::into_machine()", self.source_type_display))
    }
}

/// One transition site in the codebase export surface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CodebaseTransition {
    /// Stable machine-local transition index.
    pub index: usize,
    /// Rust method name emitted by Statum.
    pub method_name: &'static str,
    /// Optional human-facing transition label.
    pub label: Option<&'static str>,
    /// Optional human-facing transition description.
    pub description: Option<&'static str>,
    /// Stable source-state index.
    pub from: usize,
    /// Stable legal target-state indices for this transition site.
    pub to: Vec<usize>,
}

impl CodebaseTransition {
    /// Human-facing edge label used by text renderers.
    pub fn display_label(&self) -> &'static str {
        self.label.unwrap_or(self.method_name)
    }
}

/// Exact relation kinds exported by the codebase document.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum CodebaseRelationKind {
    StatePayload,
    MachineField,
    TransitionParam,
}

/// Why one exact relation was inferred.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum CodebaseRelationBasis {
    DirectTypeSyntax,
    DeclaredReferenceType,
}

/// One exact relation source in the codebase export surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum CodebaseRelationSource {
    StatePayload {
        machine: usize,
        state: usize,
        field_name: Option<&'static str>,
    },
    MachineField {
        machine: usize,
        field_name: Option<&'static str>,
        field_index: usize,
    },
    TransitionParam {
        machine: usize,
        transition: usize,
        param_index: usize,
        param_name: Option<&'static str>,
    },
}

/// One resolved exact relation in the codebase export surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CodebaseRelation {
    /// Stable codebase-local relation index.
    pub index: usize,
    /// Exact relation kind.
    pub kind: CodebaseRelationKind,
    /// Why Statum considered this relation exact.
    pub basis: CodebaseRelationBasis,
    /// Exact source location for this relation.
    pub source: CodebaseRelationSource,
    /// Resolved target machine index.
    pub target_machine: usize,
    /// Resolved target state index.
    pub target_state: usize,
    /// Declared nominal reference type when this relation came through
    /// `#[machine_ref(...)]`.
    pub declared_reference_type: Option<&'static str>,
}

type ResolvedRelationTarget = (usize, usize, Option<&'static str>);

/// One resolved static cross-machine payload link.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CodebaseLink {
    /// Stable codebase-local link index.
    pub index: usize,
    /// Source machine index.
    pub from_machine: usize,
    /// Source state index within `from_machine`.
    pub from_state: usize,
    /// Named field for named payloads; `None` for tuple payloads.
    pub field_name: Option<&'static str>,
    /// Target machine index.
    pub to_machine: usize,
    /// Target state index within `to_machine`.
    pub to_state: usize,
}

impl CodebaseLink {
    /// Human-facing link label used by text renderers.
    pub fn display_label(&self) -> &'static str {
        self.field_name.unwrap_or("state_data")
    }
}

/// Error returned when a linked machine inventory cannot be exported into a
/// stable codebase document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodebaseDocError {
    /// One linked machine family appears more than once in the inventory.
    DuplicateMachine { machine: &'static str },
    /// One linked machine exports no states.
    EmptyStateList { machine: &'static str },
    /// One state name appears more than once in one machine.
    DuplicateStateName {
        machine: &'static str,
        state: &'static str,
    },
    /// One source state declares the same transition method name more than once.
    DuplicateTransitionSite {
        machine: &'static str,
        state: &'static str,
        transition: &'static str,
    },
    /// One transition source state is not present in the machine state list.
    MissingSourceState {
        machine: &'static str,
        transition: &'static str,
    },
    /// One transition target state is not present in the machine state list.
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
    /// One validator-entry surface points at a machine missing from the linked
    /// machine inventory.
    MissingValidatorMachine {
        machine: &'static str,
        source_module_path: &'static str,
        source_type_display: &'static str,
    },
    /// One validator-entry surface points at a target state missing from the
    /// linked machine state list.
    MissingValidatorTargetState {
        machine: &'static str,
        source_module_path: &'static str,
        source_type_display: &'static str,
        state: &'static str,
    },
    /// One validator-entry surface declares no target states.
    EmptyValidatorTargetSet {
        machine: &'static str,
        source_module_path: &'static str,
        source_type_display: &'static str,
    },
    /// One validator-entry surface lists the same target state more than once.
    DuplicateValidatorTargetState {
        machine: &'static str,
        source_module_path: &'static str,
        source_type_display: &'static str,
        state: &'static str,
    },
    /// One validator-entry surface appears more than once for the same machine
    /// and impl site.
    DuplicateValidatorEntry {
        machine: &'static str,
        source_module_path: &'static str,
        source_type_display: &'static str,
    },
    /// One declared `#[machine_ref(...)]` type appears more than once for the
    /// same compiler-resolved nominal type identity.
    DuplicateReferenceTypeDeclaration {
        rust_type_path: &'static str,
        resolved_type_name: &'static str,
    },
    /// One declared `#[machine_ref(...)]` target machine is missing from the
    /// linked machine inventory.
    MissingReferenceTypeTargetMachine {
        rust_type_path: &'static str,
        target_machine_path: String,
        target_state: &'static str,
    },
    /// One declared `#[machine_ref(...)]` target state is missing from the
    /// linked machine inventory.
    MissingReferenceTypeTargetState {
        rust_type_path: &'static str,
        target_machine_path: String,
        target_state: &'static str,
    },
    /// One declared `#[machine_ref(...)]` target resolves to multiple linked
    /// machines.
    AmbiguousReferenceTypeTarget {
        rust_type_path: &'static str,
        target_machine_path: String,
        target_state: &'static str,
    },
    /// One linked exact relation cannot resolve its source machine.
    MissingRelationMachine {
        machine_path: String,
        relation: String,
    },
    /// One linked exact relation matches multiple source machines.
    AmbiguousRelationMachine {
        machine_path: String,
        relation: String,
    },
    /// One linked exact relation points at a source state missing from the
    /// resolved source machine.
    MissingRelationSourceState {
        machine: &'static str,
        state: &'static str,
        relation: String,
    },
    /// One linked exact relation points at a transition site missing from the
    /// resolved source machine.
    MissingRelationTransition {
        machine: &'static str,
        state: &'static str,
        transition: &'static str,
    },
    /// One linked exact relation matches multiple target machines.
    AmbiguousRelationTarget {
        relation: String,
        target_machine_path: String,
        target_state: &'static str,
    },
    /// One static payload link points at a source state missing from the
    /// machine state list.
    MissingStaticLinkSourceState {
        machine: &'static str,
        state: &'static str,
    },
    /// One static payload link matches multiple linked machine families.
    AmbiguousStaticLink {
        machine: &'static str,
        state: &'static str,
        field_name: Option<&'static str>,
        target_machine_path: String,
        target_state: &'static str,
    },
}

impl core::fmt::Display for CodebaseDocError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DuplicateMachine { machine } => write!(
                formatter,
                "linked codebase export cannot merge duplicate machine path `{machine}`. \
This usually means multiple linked crates define machines at the same crate-local module path. \
Whole-workspace export treats that path as the machine identity, so the merge would be ambiguous. \
Fix: rerun with `--package` to export one crate, or move one machine to a distinct module path."
            ),
            Self::EmptyStateList { machine } => {
                write!(formatter, "linked machine `{machine}` contains no states")
            }
            Self::DuplicateStateName { machine, state } => write!(
                formatter,
                "linked machine `{machine}` contains duplicate state `{state}`"
            ),
            Self::DuplicateTransitionSite {
                machine,
                state,
                transition,
            } => write!(
                formatter,
                "linked machine `{machine}` contains duplicate transition site `{state}::{transition}`"
            ),
            Self::MissingSourceState { machine, transition } => write!(
                formatter,
                "linked machine `{machine}` contains transition `{transition}` whose source state is missing from the state list"
            ),
            Self::MissingTargetState { machine, transition } => write!(
                formatter,
                "linked machine `{machine}` contains transition `{transition}` whose target state is missing from the state list"
            ),
            Self::EmptyTargetSet { machine, transition } => write!(
                formatter,
                "linked machine `{machine}` contains transition `{transition}` with no target states"
            ),
            Self::DuplicateTargetState {
                machine,
                transition,
                state,
            } => write!(
                formatter,
                "linked machine `{machine}` contains transition `{transition}` with duplicate target state `{state}`"
            ),
            Self::MissingValidatorMachine {
                machine,
                source_module_path,
                source_type_display,
            } => write!(
                formatter,
                "linked validator entry `{source_type_display}::into_machine()` from module `{source_module_path}` points at missing machine `{machine}`"
            ),
            Self::MissingValidatorTargetState {
                machine,
                source_module_path,
                source_type_display,
                state,
            } => write!(
                formatter,
                "linked validator entry `{source_type_display}::into_machine()` from module `{source_module_path}` points at missing state `{machine}::{state}`"
            ),
            Self::EmptyValidatorTargetSet {
                machine,
                source_module_path,
                source_type_display,
            } => write!(
                formatter,
                "linked validator entry `{source_type_display}::into_machine()` from module `{source_module_path}` for machine `{machine}` contains no target states"
            ),
            Self::DuplicateValidatorTargetState {
                machine,
                source_module_path,
                source_type_display,
                state,
            } => write!(
                formatter,
                "linked validator entry `{source_type_display}::into_machine()` from module `{source_module_path}` for machine `{machine}` contains duplicate target state `{state}`"
            ),
            Self::DuplicateValidatorEntry {
                machine,
                source_module_path,
                source_type_display,
            } => write!(
                formatter,
                "linked validator entry `{source_type_display}::into_machine()` from module `{source_module_path}` appears more than once for machine `{machine}`"
            ),
            Self::DuplicateReferenceTypeDeclaration {
                rust_type_path,
                resolved_type_name,
            } => write!(
                formatter,
                "linked machine reference type `{rust_type_path}` appears more than once for resolved type identity `{resolved_type_name}`"
            ),
            Self::MissingReferenceTypeTargetMachine {
                rust_type_path,
                target_machine_path,
                target_state,
            } => write!(
                formatter,
                "linked machine reference type `{rust_type_path}` points at missing target `{target_machine_path}<{target_state}>`"
            ),
            Self::MissingReferenceTypeTargetState {
                rust_type_path,
                target_machine_path,
                target_state,
            } => write!(
                formatter,
                "linked machine reference type `{rust_type_path}` points at missing target state `{target_machine_path}::{target_state}`"
            ),
            Self::AmbiguousReferenceTypeTarget {
                rust_type_path,
                target_machine_path,
                target_state,
            } => write!(
                formatter,
                "linked machine reference type `{rust_type_path}` ambiguously matches target `{target_machine_path}<{target_state}>`"
            ),
            Self::MissingRelationMachine {
                machine_path,
                relation,
            } => write!(
                formatter,
                "linked exact relation `{relation}` points at missing source machine `{machine_path}`"
            ),
            Self::AmbiguousRelationMachine {
                machine_path,
                relation,
            } => write!(
                formatter,
                "linked exact relation `{relation}` ambiguously matches source machine `{machine_path}`"
            ),
            Self::MissingRelationSourceState {
                machine,
                state,
                relation,
            } => write!(
                formatter,
                "linked exact relation `{relation}` points at missing source state `{machine}::{state}`"
            ),
            Self::MissingRelationTransition {
                machine,
                state,
                transition,
            } => write!(
                formatter,
                "linked exact relation for transition `{machine}::{state}::{transition}` points at a transition site missing from the exported machine graph"
            ),
            Self::AmbiguousRelationTarget {
                relation,
                target_machine_path,
                target_state,
            } => write!(
                formatter,
                "linked exact relation `{relation}` ambiguously matches target `{target_machine_path}<{target_state}>`"
            ),
            Self::MissingStaticLinkSourceState { machine, state } => write!(
                formatter,
                "linked machine `{machine}` contains a static payload link from missing source state `{state}`"
            ),
            Self::AmbiguousStaticLink {
                machine,
                state,
                field_name,
                target_machine_path,
                target_state,
            } => match field_name {
                Some(field_name) => write!(
                    formatter,
                    "linked machine `{machine}` state `{state}` field `{field_name}` ambiguously matches static target `{target_machine_path}<{target_state}>`"
                ),
                None => write!(
                    formatter,
                    "linked machine `{machine}` state `{state}` ambiguously matches static target `{target_machine_path}<{target_state}>`"
                ),
            },
        }
    }
}

impl std::error::Error for CodebaseDocError {}

fn build_machine(
    machine_index: usize,
    linked: LinkedMachineGraph,
) -> Result<(CodebaseMachine, Vec<&'static StaticMachineLinkDescriptor>), CodebaseDocError> {
    if linked.states.is_empty() {
        return Err(CodebaseDocError::EmptyStateList {
            machine: linked.machine.rust_type_path,
        });
    }

    let mut states = Vec::with_capacity(linked.states.len());
    let mut state_positions = HashMap::with_capacity(linked.states.len());
    for (index, state) in linked.states.iter().enumerate() {
        if state_positions.insert(state.rust_name, index).is_some() {
            return Err(CodebaseDocError::DuplicateStateName {
                machine: linked.machine.rust_type_path,
                state: state.rust_name,
            });
        }

        states.push(CodebaseState {
            index,
            rust_name: state.rust_name,
            label: state.label,
            description: state.description,
            has_data: state.has_data,
            direct_construction_available: state.direct_construction_available,
            is_graph_root: true,
        });
    }

    let mut transitions = linked.transitions.as_slice().to_vec();
    transitions.sort_by(|left, right| compare_transitions(&state_positions, left, right));

    let mut exported_transitions = Vec::with_capacity(transitions.len());
    let mut incoming = HashSet::new();
    let mut seen_sites = HashSet::with_capacity(transitions.len());

    for (index, transition) in transitions.iter().enumerate() {
        let Some(&from) = state_positions.get(transition.from) else {
            return Err(CodebaseDocError::MissingSourceState {
                machine: linked.machine.rust_type_path,
                transition: transition.method_name,
            });
        };
        if !seen_sites.insert((transition.from, transition.method_name)) {
            return Err(CodebaseDocError::DuplicateTransitionSite {
                machine: linked.machine.rust_type_path,
                state: transition.from,
                transition: transition.method_name,
            });
        }
        if transition.to.is_empty() {
            return Err(CodebaseDocError::EmptyTargetSet {
                machine: linked.machine.rust_type_path,
                transition: transition.method_name,
            });
        }

        let mut to = Vec::with_capacity(transition.to.len());
        let mut seen_targets = HashSet::with_capacity(transition.to.len());
        for target in transition.to {
            let Some(&target_index) = state_positions.get(target) else {
                return Err(CodebaseDocError::MissingTargetState {
                    machine: linked.machine.rust_type_path,
                    transition: transition.method_name,
                });
            };
            if !seen_targets.insert(*target) {
                return Err(CodebaseDocError::DuplicateTargetState {
                    machine: linked.machine.rust_type_path,
                    transition: transition.method_name,
                    state: target,
                });
            }
            incoming.insert(target_index);
            to.push(target_index);
        }

        exported_transitions.push(CodebaseTransition {
            index,
            method_name: transition.method_name,
            label: transition.label,
            description: transition.description,
            from,
            to,
        });
    }

    for state in &mut states {
        state.is_graph_root = !incoming.contains(&state.index);
    }

    Ok((
        CodebaseMachine {
            index: machine_index,
            module_path: linked.machine.module_path,
            rust_type_path: linked.machine.rust_type_path,
            label: linked.label,
            description: linked.description,
            states,
            transitions: exported_transitions,
            validator_entries: Vec::new(),
        },
        linked.static_links.iter().collect(),
    ))
}

fn compare_transitions(
    state_positions: &HashMap<&'static str, usize>,
    left: &statum::LinkedTransitionDescriptor,
    right: &statum::LinkedTransitionDescriptor,
) -> core::cmp::Ordering {
    transition_sort_key(state_positions, left).cmp(&transition_sort_key(state_positions, right))
}

fn transition_sort_key(
    state_positions: &HashMap<&'static str, usize>,
    transition: &statum::LinkedTransitionDescriptor,
) -> (Option<usize>, &'static str, &'static [&'static str]) {
    (
        state_positions.get(transition.from).copied(),
        transition.method_name,
        transition.to,
    )
}

fn resolve_static_links(
    machines: &[CodebaseMachine],
    static_links: &[Vec<&'static StaticMachineLinkDescriptor>],
) -> Result<Vec<CodebaseLink>, CodebaseDocError> {
    let mut links = Vec::new();

    for (machine_index, machine_links) in static_links.iter().enumerate() {
        let machine = &machines[machine_index];
        for link in machine_links {
            let Some(from_state) = machine
                .state_named(link.from_state)
                .map(|state| state.index)
            else {
                return Err(CodebaseDocError::MissingStaticLinkSourceState {
                    machine: machine.rust_type_path,
                    state: link.from_state,
                });
            };

            let candidates = machines
                .iter()
                .filter_map(|candidate| {
                    if !path_suffix_matches(candidate.rust_type_path, link.to_machine_path) {
                        return None;
                    }

                    candidate
                        .state_named(link.to_state)
                        .map(|target_state| (candidate.index, target_state.index))
                })
                .collect::<Vec<_>>();

            match candidates.as_slice() {
                [] => {}
                [(to_machine, to_state)] => links.push(CodebaseLink {
                    index: links.len(),
                    from_machine: machine_index,
                    from_state,
                    field_name: link.field_name,
                    to_machine: *to_machine,
                    to_state: *to_state,
                }),
                _ => {
                    return Err(CodebaseDocError::AmbiguousStaticLink {
                        machine: machine.rust_type_path,
                        state: link.from_state,
                        field_name: link.field_name,
                        target_machine_path: link.to_machine_path.join("::"),
                        target_state: link.to_state,
                    });
                }
            }
        }
    }

    Ok(links)
}

#[derive(Clone, Copy)]
struct ResolvedReferenceTypeTarget {
    rust_type_path: &'static str,
    target_machine: usize,
    target_state: usize,
}

fn resolve_relations(
    machines: &[CodebaseMachine],
    relations: &'static [LinkedRelationDescriptor],
    reference_types: &'static [LinkedReferenceTypeDescriptor],
) -> Result<Vec<CodebaseRelation>, CodebaseDocError> {
    let reference_types = resolve_reference_type_targets(machines, reference_types)?;
    let mut relations = relations.to_vec();
    relations.sort_by(compare_relations);

    let exact_machine_positions = machines
        .iter()
        .map(|machine| (machine.rust_type_path, machine.index))
        .collect::<HashMap<_, _>>();
    let mut exported = Vec::new();

    for relation in relations {
        let source_machine = resolve_relation_source_machine(
            machines,
            &exact_machine_positions,
            relation.machine.rust_type_path,
            &relation_summary(&relation),
        )?;
        let machine = &machines[source_machine];
        let source = match relation.source {
            LinkedRelationSource::StatePayload { state, field_name } => {
                let Some(state_index) = machine.state_named(state).map(|state| state.index) else {
                    return Err(CodebaseDocError::MissingRelationSourceState {
                        machine: machine.rust_type_path,
                        state,
                        relation: relation_summary(&relation),
                    });
                };
                CodebaseRelationSource::StatePayload {
                    machine: machine.index,
                    state: state_index,
                    field_name,
                }
            }
            LinkedRelationSource::MachineField {
                field_name,
                field_index,
            } => CodebaseRelationSource::MachineField {
                machine: machine.index,
                field_name,
                field_index,
            },
            LinkedRelationSource::TransitionParam {
                state,
                transition,
                param_index,
                param_name,
            } => {
                let Some(transition_index) = machine
                    .transition_site(state, transition)
                    .map(|transition| transition.index)
                else {
                    return Err(CodebaseDocError::MissingRelationTransition {
                        machine: machine.rust_type_path,
                        state,
                        transition,
                    });
                };
                CodebaseRelationSource::TransitionParam {
                    machine: machine.index,
                    transition: transition_index,
                    param_index,
                    param_name,
                }
            }
        };

        let resolved_target = match relation.target {
            LinkedRelationTarget::DirectMachine {
                machine_path,
                state,
            } => resolve_optional_target_machine(
                machines,
                machine_path,
                state,
                &relation_summary(&relation),
                true,
            )?,
            LinkedRelationTarget::DeclaredReferenceType { resolved_type_name } => reference_types
                .get(resolved_type_name())
                .copied()
                .map(|target| {
                    (
                        target.target_machine,
                        target.target_state,
                        Some(target.rust_type_path),
                    )
                }),
        };
        let Some((target_machine, target_state, declared_reference_type)) = resolved_target else {
            continue;
        };

        exported.push(CodebaseRelation {
            index: exported.len(),
            kind: map_relation_kind(relation.kind),
            basis: map_relation_basis(relation.basis),
            source,
            target_machine,
            target_state,
            declared_reference_type,
        });
    }

    Ok(exported)
}

fn resolve_reference_type_targets(
    machines: &[CodebaseMachine],
    reference_types: &'static [LinkedReferenceTypeDescriptor],
) -> Result<HashMap<&'static str, ResolvedReferenceTypeTarget>, CodebaseDocError> {
    let mut reference_types = reference_types.to_vec();
    reference_types.sort_by(compare_reference_types);

    let mut resolved = HashMap::with_capacity(reference_types.len());
    for reference_type in reference_types {
        let resolved_type_name = (reference_type.resolved_type_name)();
        if resolved.contains_key(resolved_type_name) {
            return Err(CodebaseDocError::DuplicateReferenceTypeDeclaration {
                rust_type_path: reference_type.rust_type_path,
                resolved_type_name,
            });
        }

        let target = resolve_required_target_machine(
            machines,
            reference_type.to_machine_path,
            reference_type.to_state,
            |target_machine_path, target_state| {
                CodebaseDocError::MissingReferenceTypeTargetMachine {
                    rust_type_path: reference_type.rust_type_path,
                    target_machine_path,
                    target_state,
                }
            },
            |target_machine_path, target_state| CodebaseDocError::MissingReferenceTypeTargetState {
                rust_type_path: reference_type.rust_type_path,
                target_machine_path,
                target_state,
            },
            |target_machine_path, target_state| CodebaseDocError::AmbiguousReferenceTypeTarget {
                rust_type_path: reference_type.rust_type_path,
                target_machine_path,
                target_state,
            },
            true,
        )?;

        resolved.insert(
            resolved_type_name,
            ResolvedReferenceTypeTarget {
                rust_type_path: reference_type.rust_type_path,
                target_machine: target.0,
                target_state: target.1,
            },
        );
    }

    Ok(resolved)
}

fn resolve_relation_source_machine(
    machines: &[CodebaseMachine],
    exact_machine_positions: &HashMap<&'static str, usize>,
    machine_path: &'static str,
    relation: &str,
) -> Result<usize, CodebaseDocError> {
    if let Some(&machine_index) = exact_machine_positions.get(machine_path) {
        return Ok(machine_index);
    }

    let candidates = machines
        .iter()
        .filter(|candidate| path_string_suffix_matches(candidate.rust_type_path, machine_path))
        .map(|candidate| candidate.index)
        .collect::<Vec<_>>();

    match candidates.as_slice() {
        [] => Err(CodebaseDocError::MissingRelationMachine {
            machine_path: machine_path.to_owned(),
            relation: relation.to_owned(),
        }),
        [machine_index] => Ok(*machine_index),
        _ => Err(CodebaseDocError::AmbiguousRelationMachine {
            machine_path: machine_path.to_owned(),
            relation: relation.to_owned(),
        }),
    }
}

fn resolve_optional_target_machine(
    machines: &[CodebaseMachine],
    machine_path: &'static [&'static str],
    state: &'static str,
    relation: &str,
    exact: bool,
) -> Result<Option<ResolvedRelationTarget>, CodebaseDocError> {
    let candidates = target_candidates(machines, machine_path, state, exact);
    match candidates.as_slice() {
        [] => Ok(None),
        [(machine_index, state_index)] => Ok(Some((*machine_index, *state_index, None))),
        _ => Err(CodebaseDocError::AmbiguousRelationTarget {
            relation: relation.to_owned(),
            target_machine_path: machine_path.join("::"),
            target_state: state,
        }),
    }
}

fn resolve_required_target_machine<FMissingMachine, FMissingState, FAmbiguous>(
    machines: &[CodebaseMachine],
    machine_path: &'static [&'static str],
    state: &'static str,
    missing_machine: FMissingMachine,
    missing_state: FMissingState,
    ambiguous: FAmbiguous,
    exact: bool,
) -> Result<(usize, usize), CodebaseDocError>
where
    FMissingMachine: FnOnce(String, &'static str) -> CodebaseDocError,
    FMissingState: FnOnce(String, &'static str) -> CodebaseDocError,
    FAmbiguous: FnOnce(String, &'static str) -> CodebaseDocError,
{
    let machine_path_string = machine_path.join("::");
    let matching_machines = machines
        .iter()
        .filter(|candidate| machine_path_matches(candidate.rust_type_path, machine_path, exact))
        .collect::<Vec<_>>();
    if matching_machines.is_empty() {
        return Err(missing_machine(machine_path_string, state));
    }

    let candidates = matching_machines
        .iter()
        .filter_map(|candidate| {
            candidate
                .state_named(state)
                .map(|target_state| (candidate.index, target_state.index))
        })
        .collect::<Vec<_>>();

    match candidates.as_slice() {
        [] => Err(missing_state(machine_path.join("::"), state)),
        [(machine_index, state_index)] => Ok((*machine_index, *state_index)),
        _ => Err(ambiguous(machine_path.join("::"), state)),
    }
}

fn target_candidates(
    machines: &[CodebaseMachine],
    machine_path: &'static [&'static str],
    state: &'static str,
    exact: bool,
) -> Vec<(usize, usize)> {
    machines
        .iter()
        .filter_map(|candidate| {
            if !machine_path_matches(candidate.rust_type_path, machine_path, exact) {
                return None;
            }

            candidate
                .state_named(state)
                .map(|target_state| (candidate.index, target_state.index))
        })
        .collect()
}

fn machine_path_matches(candidate: &str, path: &[&'static str], exact: bool) -> bool {
    if exact {
        return candidate.split("::").eq(path.iter().copied());
    }

    path_suffix_matches(candidate, path)
}

fn map_relation_kind(kind: LinkedRelationKind) -> CodebaseRelationKind {
    match kind {
        LinkedRelationKind::StatePayload => CodebaseRelationKind::StatePayload,
        LinkedRelationKind::MachineField => CodebaseRelationKind::MachineField,
        LinkedRelationKind::TransitionParam => CodebaseRelationKind::TransitionParam,
    }
}

fn map_relation_basis(basis: LinkedRelationBasis) -> CodebaseRelationBasis {
    match basis {
        LinkedRelationBasis::DirectTypeSyntax => CodebaseRelationBasis::DirectTypeSyntax,
        LinkedRelationBasis::DeclaredReferenceType => CodebaseRelationBasis::DeclaredReferenceType,
    }
}

fn compare_reference_types(
    left: &LinkedReferenceTypeDescriptor,
    right: &LinkedReferenceTypeDescriptor,
) -> core::cmp::Ordering {
    (left.resolved_type_name)()
        .cmp((right.resolved_type_name)())
        .then_with(|| left.rust_type_path.cmp(right.rust_type_path))
        .then_with(|| left.to_machine_path.cmp(right.to_machine_path))
        .then_with(|| left.to_state.cmp(right.to_state))
}

fn compare_relations(
    left: &LinkedRelationDescriptor,
    right: &LinkedRelationDescriptor,
) -> core::cmp::Ordering {
    left.machine
        .rust_type_path
        .cmp(right.machine.rust_type_path)
        .then_with(|| compare_relation_sources(&left.source, &right.source))
        .then_with(|| {
            linked_relation_kind_rank(left.kind).cmp(&linked_relation_kind_rank(right.kind))
        })
        .then_with(|| {
            linked_relation_basis_rank(left.basis).cmp(&linked_relation_basis_rank(right.basis))
        })
        .then_with(|| compare_relation_targets(&left.target, &right.target))
}

fn compare_relation_sources(
    left: &LinkedRelationSource,
    right: &LinkedRelationSource,
) -> core::cmp::Ordering {
    match (left, right) {
        (
            LinkedRelationSource::StatePayload {
                state: left_state,
                field_name: left_field,
            },
            LinkedRelationSource::StatePayload {
                state: right_state,
                field_name: right_field,
            },
        ) => left_state
            .cmp(right_state)
            .then_with(|| left_field.cmp(right_field)),
        (
            LinkedRelationSource::MachineField {
                field_name: left_field,
                field_index: left_index,
            },
            LinkedRelationSource::MachineField {
                field_name: right_field,
                field_index: right_index,
            },
        ) => left_index
            .cmp(right_index)
            .then_with(|| left_field.cmp(right_field)),
        (
            LinkedRelationSource::TransitionParam {
                state: left_state,
                transition: left_transition,
                param_index: left_index,
                param_name: left_name,
            },
            LinkedRelationSource::TransitionParam {
                state: right_state,
                transition: right_transition,
                param_index: right_index,
                param_name: right_name,
            },
        ) => left_state
            .cmp(right_state)
            .then_with(|| left_transition.cmp(right_transition))
            .then_with(|| left_index.cmp(right_index))
            .then_with(|| left_name.cmp(right_name)),
        (left, right) => linked_relation_source_rank(left).cmp(&linked_relation_source_rank(right)),
    }
}

fn compare_relation_targets(
    left: &LinkedRelationTarget,
    right: &LinkedRelationTarget,
) -> core::cmp::Ordering {
    match (left, right) {
        (
            LinkedRelationTarget::DirectMachine {
                machine_path: left_path,
                state: left_state,
            },
            LinkedRelationTarget::DirectMachine {
                machine_path: right_path,
                state: right_state,
            },
        ) => left_path
            .cmp(right_path)
            .then_with(|| left_state.cmp(right_state)),
        (
            LinkedRelationTarget::DeclaredReferenceType {
                resolved_type_name: left_name,
            },
            LinkedRelationTarget::DeclaredReferenceType {
                resolved_type_name: right_name,
            },
        ) => left_name().cmp(right_name()),
        (left, right) => linked_relation_target_rank(left).cmp(&linked_relation_target_rank(right)),
    }
}

fn linked_relation_kind_rank(kind: LinkedRelationKind) -> u8 {
    match kind {
        LinkedRelationKind::StatePayload => 0,
        LinkedRelationKind::MachineField => 1,
        LinkedRelationKind::TransitionParam => 2,
    }
}

fn linked_relation_basis_rank(basis: LinkedRelationBasis) -> u8 {
    match basis {
        LinkedRelationBasis::DirectTypeSyntax => 0,
        LinkedRelationBasis::DeclaredReferenceType => 1,
    }
}

fn linked_relation_source_rank(source: &LinkedRelationSource) -> u8 {
    match source {
        LinkedRelationSource::StatePayload { .. } => 0,
        LinkedRelationSource::MachineField { .. } => 1,
        LinkedRelationSource::TransitionParam { .. } => 2,
    }
}

fn linked_relation_target_rank(target: &LinkedRelationTarget) -> u8 {
    match target {
        LinkedRelationTarget::DirectMachine { .. } => 0,
        LinkedRelationTarget::DeclaredReferenceType { .. } => 1,
    }
}

fn relation_summary(relation: &LinkedRelationDescriptor) -> String {
    match relation.source {
        LinkedRelationSource::StatePayload { state, field_name } => match field_name {
            Some(field_name) => format!(
                "{} state payload {}::{}",
                relation.machine.rust_type_path, state, field_name
            ),
            None => format!(
                "{} state payload {}",
                relation.machine.rust_type_path, state
            ),
        },
        LinkedRelationSource::MachineField {
            field_name,
            field_index,
        } => match field_name {
            Some(field_name) => format!(
                "{} machine field {}",
                relation.machine.rust_type_path, field_name
            ),
            None => format!(
                "{} machine field #{}",
                relation.machine.rust_type_path, field_index
            ),
        },
        LinkedRelationSource::TransitionParam {
            state,
            transition,
            param_index,
            param_name,
        } => match param_name {
            Some(param_name) => format!(
                "{} transition param {}::{}({})",
                relation.machine.rust_type_path, state, transition, param_name
            ),
            None => format!(
                "{} transition param {}::{}[#{}]",
                relation.machine.rust_type_path, state, transition, param_index
            ),
        },
    }
}

fn resolve_validator_entries(
    machines: &mut [CodebaseMachine],
    validator_entries: &'static [LinkedValidatorEntryDescriptor],
) -> Result<usize, CodebaseDocError> {
    let mut validator_entries = validator_entries.to_vec();
    validator_entries.sort_by(compare_validator_entries);

    let machine_positions = machines
        .iter()
        .map(|machine| (machine.rust_type_path, machine.index))
        .collect::<HashMap<_, _>>();
    let mut seen_entries = HashSet::with_capacity(validator_entries.len());

    for entry in validator_entries {
        let Some(&machine_index) = machine_positions.get(entry.machine.rust_type_path) else {
            return Err(CodebaseDocError::MissingValidatorMachine {
                machine: entry.machine.rust_type_path,
                source_module_path: entry.source_module_path,
                source_type_display: entry.source_type_display,
            });
        };
        if !seen_entries.insert((
            entry.machine.rust_type_path,
            entry.source_module_path,
            entry.source_type_display,
        )) {
            return Err(CodebaseDocError::DuplicateValidatorEntry {
                machine: entry.machine.rust_type_path,
                source_module_path: entry.source_module_path,
                source_type_display: entry.source_type_display,
            });
        }
        if entry.target_states.is_empty() {
            return Err(CodebaseDocError::EmptyValidatorTargetSet {
                machine: entry.machine.rust_type_path,
                source_module_path: entry.source_module_path,
                source_type_display: entry.source_type_display,
            });
        }

        let machine = &mut machines[machine_index];
        let mut target_states = Vec::with_capacity(entry.target_states.len());
        let mut seen_target_states = HashSet::with_capacity(entry.target_states.len());

        for target_state in entry.target_states {
            let Some(target_index) = machine.state_named(target_state).map(|state| state.index)
            else {
                return Err(CodebaseDocError::MissingValidatorTargetState {
                    machine: machine.rust_type_path,
                    source_module_path: entry.source_module_path,
                    source_type_display: entry.source_type_display,
                    state: target_state,
                });
            };
            if !seen_target_states.insert(*target_state) {
                return Err(CodebaseDocError::DuplicateValidatorTargetState {
                    machine: machine.rust_type_path,
                    source_module_path: entry.source_module_path,
                    source_type_display: entry.source_type_display,
                    state: target_state,
                });
            }
            target_states.push(target_index);
        }

        target_states.sort_unstable();

        machine.validator_entries.push(CodebaseValidatorEntry {
            index: machine.validator_entries.len(),
            source_module_path: entry.source_module_path,
            source_type_display: entry.source_type_display,
            target_states,
        });
    }

    Ok(total_validator_entries(machines))
}

fn total_validator_entries(machines: &[CodebaseMachine]) -> usize {
    machines
        .iter()
        .map(|machine| machine.validator_entries.len())
        .sum()
}

fn compare_validator_entries(
    left: &LinkedValidatorEntryDescriptor,
    right: &LinkedValidatorEntryDescriptor,
) -> core::cmp::Ordering {
    left.machine
        .rust_type_path
        .cmp(right.machine.rust_type_path)
        .then_with(|| left.source_module_path.cmp(right.source_module_path))
        .then_with(|| left.source_type_display.cmp(right.source_type_display))
        .then_with(|| left.target_states.cmp(right.target_states))
}

fn path_suffix_matches(candidate: &str, suffix: &[&'static str]) -> bool {
    let candidate = candidate.split("::").collect::<Vec<_>>();
    candidate.ends_with(suffix)
}

fn path_string_suffix_matches(candidate: &str, suffix: &str) -> bool {
    let suffix = suffix
        .split("::")
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if suffix.is_empty() {
        return false;
    }

    let candidate = candidate.split("::").collect::<Vec<_>>();
    candidate.ends_with(&suffix)
}
