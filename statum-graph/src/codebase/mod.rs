use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use serde::Serialize;
use statum::{LinkedMachineGraph, LinkedValidatorEntryDescriptor, StaticMachineLinkDescriptor};

pub mod render;

/// Stable export model for the linked compiled machine inventory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CodebaseDoc {
    machines: Vec<CodebaseMachine>,
    links: Vec<CodebaseLink>,
}

impl CodebaseDoc {
    /// Builds a combined codebase document from every linked machine visible to
    /// the current build.
    pub fn linked() -> Result<Self, CodebaseDocError> {
        Self::try_from_linked_with_validator_entries(
            statum::linked_machines(),
            statum::linked_validator_entries(),
        )
    }

    /// Builds a combined codebase document from an explicit linked machine
    /// inventory.
    pub fn try_from_linked(
        linked: &'static [LinkedMachineGraph],
    ) -> Result<Self, CodebaseDocError> {
        Self::try_from_linked_with_validator_entries(linked, &[])
    }

    /// Builds a combined codebase document from explicit linked machine and
    /// validator-entry inventories.
    pub fn try_from_linked_with_validator_entries(
        linked: &'static [LinkedMachineGraph],
        validator_entries: &'static [LinkedValidatorEntryDescriptor],
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

        debug_assert_eq!(
            resolved_validator_entries,
            total_validator_entries(&machines)
        );

        Ok(Self { machines, links })
    }

    /// Exported machines in stable codebase order.
    pub fn machines(&self) -> &[CodebaseMachine] {
        &self.machines
    }

    /// Resolved static cross-machine links in stable order.
    pub fn links(&self) -> &[CodebaseLink] {
        &self.links
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
