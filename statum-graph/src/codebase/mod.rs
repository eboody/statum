use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;
use statum::{
    LinkedMachineGraph, LinkedReferenceTypeDescriptor, LinkedRelationBasis,
    LinkedRelationDescriptor, LinkedRelationKind, LinkedRelationSource, LinkedRelationTarget,
    LinkedValidatorEntryDescriptor, LinkedViaRouteDescriptor, StaticMachineLinkDescriptor,
};

pub mod render;

/// Stable export model for the linked compiled machine inventory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CodebaseDoc {
    machines: Vec<CodebaseMachine>,
    links: Vec<CodebaseLink>,
    relations: Vec<CodebaseRelation>,
    #[serde(skip)]
    relation_groups: Vec<CodebaseMachineRelationGroup>,
    #[serde(skip)]
    relation_index: CodebaseRelationIndex,
}

impl CodebaseDoc {
    /// Builds a combined codebase document from every linked machine visible to
    /// the current build.
    pub fn linked() -> Result<Self, CodebaseDocError> {
        Self::try_from_linked_with_inventories(
            statum::linked_machines(),
            statum::linked_validator_entries(),
            statum::linked_relations(),
            statum::linked_via_routes(),
            statum::linked_reference_types(),
        )
    }

    /// Builds a combined codebase document from an explicit linked machine
    /// inventory.
    pub fn try_from_linked(
        linked: &'static [LinkedMachineGraph],
    ) -> Result<Self, CodebaseDocError> {
        Self::try_from_linked_with_inventories(linked, &[], &[], &[], &[])
    }

    /// Builds a combined codebase document from explicit linked machine and
    /// validator-entry inventories.
    pub fn try_from_linked_with_validator_entries(
        linked: &'static [LinkedMachineGraph],
        validator_entries: &'static [LinkedValidatorEntryDescriptor],
    ) -> Result<Self, CodebaseDocError> {
        Self::try_from_linked_with_inventories(linked, validator_entries, &[], &[], &[])
    }

    fn try_from_linked_with_inventories(
        linked: &'static [LinkedMachineGraph],
        validator_entries: &'static [LinkedValidatorEntryDescriptor],
        relations: &'static [LinkedRelationDescriptor],
        via_routes: &'static [LinkedViaRouteDescriptor],
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
        let relations = resolve_relations(&machines, relations, via_routes, reference_types)?;
        let relation_groups = build_machine_relation_groups(&relations);
        let relation_index = CodebaseRelationIndex::new(&machines, &relations);

        debug_assert_eq!(
            resolved_validator_entries,
            total_validator_entries(&machines)
        );

        Ok(Self {
            machines,
            links,
            relations,
            relation_groups,
            relation_index,
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

    /// Resolved exact static relations in stable order.
    pub fn relations(&self) -> &[CodebaseRelation] {
        &self.relations
    }

    /// Returns one exported machine by its stable codebase index.
    pub fn machine(&self, index: usize) -> Option<&CodebaseMachine> {
        self.machines.get(index)
    }

    /// Returns one exported relation by its stable codebase index.
    pub fn relation(&self, index: usize) -> Option<&CodebaseRelation> {
        self.relations.get(index)
    }

    /// Groups exact relations by source and target machine for renderer and
    /// inspector use.
    pub fn machine_relation_groups(&self) -> &[CodebaseMachineRelationGroup] {
        &self.relation_groups
    }

    /// Groups exact relations that are owned by composition machines.
    pub fn composition_relation_groups(&self) -> Vec<CodebaseMachineRelationGroup> {
        self.machine_relation_groups()
            .iter()
            .filter(|group| group.is_composition_owned() && group.from_machine != group.to_machine)
            .cloned()
            .collect()
    }

    /// Exact relations whose source belongs to `machine_index`.
    pub fn outbound_relations_for_machine(
        &self,
        machine_index: usize,
    ) -> impl Iterator<Item = &CodebaseRelation> + '_ {
        self.relation_index
            .outbound_machine(machine_index)
            .iter()
            .filter_map(|index| self.relation(*index))
    }

    /// Exact relations whose target belongs to `machine_index`.
    pub fn inbound_relations_for_machine(
        &self,
        machine_index: usize,
    ) -> impl Iterator<Item = &CodebaseRelation> + '_ {
        self.relation_index
            .inbound_machine(machine_index)
            .iter()
            .filter_map(|index| self.relation(*index))
    }

    /// Exact relations whose source belongs to one exported state.
    pub fn outbound_relations_for_state(
        &self,
        machine_index: usize,
        state_index: usize,
    ) -> impl Iterator<Item = &CodebaseRelation> + '_ {
        self.relation_index
            .outbound_state(machine_index, state_index)
            .iter()
            .filter_map(|index| self.relation(*index))
    }

    /// Exact relations whose target belongs to one exported state.
    pub fn inbound_relations_for_state(
        &self,
        machine_index: usize,
        state_index: usize,
    ) -> impl Iterator<Item = &CodebaseRelation> + '_ {
        self.relation_index
            .inbound_state(machine_index, state_index)
            .iter()
            .filter_map(|index| self.relation(*index))
    }

    /// Exact relations whose source belongs to one exported transition site.
    pub fn outbound_relations_for_transition(
        &self,
        machine_index: usize,
        transition_index: usize,
    ) -> impl Iterator<Item = &CodebaseRelation> + '_ {
        self.relation_index
            .outbound_transition(machine_index, transition_index)
            .iter()
            .filter_map(|index| self.relation(*index))
    }

    /// Exact relations whose target belongs to one exported transition site.
    ///
    /// The current exact relation surface never targets transitions, so this
    /// iterator is always empty. It exists so the inspector can use the same
    /// navigation API shape for machines, states, and transitions.
    pub fn inbound_relations_for_transition(
        &self,
        machine_index: usize,
        transition_index: usize,
    ) -> impl Iterator<Item = &CodebaseRelation> + '_ {
        self.relation_index
            .inbound_transition(machine_index, transition_index)
            .iter()
            .filter_map(|index| self.relation(*index))
    }

    /// Resolves one exact relation into typed source and target references for
    /// downstream consumers such as the inspector TUI.
    pub fn relation_detail(&self, index: usize) -> Option<CodebaseRelationDetail<'_>> {
        let relation = self.relation(index)?;
        let source_machine = self.machine(relation.source_machine())?;
        let source_state = relation
            .source_state()
            .and_then(|state| source_machine.state(state));
        let source_transition = relation
            .source_transition()
            .and_then(|transition| source_machine.transition(transition));
        let target_machine = self.machine(relation.target_machine)?;
        let target_state = target_machine.state(relation.target_state)?;
        let attested_via_producers = relation
            .attested_via
            .as_ref()
            .map(|route| {
                route
                    .producers
                    .iter()
                    .filter_map(|producer| {
                        let machine = self.machine(producer.machine)?;
                        let state = machine.state(producer.state)?;
                        let transition = machine.transition(producer.transition)?;
                        Some(CodebaseAttestedProducerDetail {
                            producer,
                            machine,
                            state,
                            transition,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let (attested_via_machine, attested_via_state, attested_via_transition) =
            if attested_via_producers.len() == 1 {
                let producer = &attested_via_producers[0];
                (
                    Some(producer.machine),
                    Some(producer.state),
                    Some(producer.transition),
                )
            } else {
                (None, None, None)
            };

        Some(CodebaseRelationDetail {
            relation,
            source_machine,
            source_state,
            source_transition,
            target_machine,
            target_state,
            attested_via_machine,
            attested_via_state,
            attested_via_transition,
            attested_via_producers,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodebaseMachineRole {
    Protocol,
    Composition,
}

impl CodebaseMachineRole {
    /// Human-facing machine-role label for inspector and renderer detail.
    pub const fn display_label(self) -> &'static str {
        match self {
            Self::Protocol => "protocol",
            Self::Composition => "composition",
        }
    }

    /// Whether this machine participates as a composition machine.
    pub const fn is_composition(self) -> bool {
        matches!(self, Self::Composition)
    }
}

impl From<statum::MachineRole> for CodebaseMachineRole {
    fn from(value: statum::MachineRole) -> Self {
        match value {
            statum::MachineRole::Protocol => Self::Protocol,
            statum::MachineRole::Composition => Self::Composition,
        }
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
    /// Whether this machine is a local protocol machine or a composition
    /// machine.
    pub role: CodebaseMachineRole,
    /// Optional human-facing machine label.
    pub label: Option<&'static str>,
    /// Optional human-facing machine description.
    pub description: Option<&'static str>,
    /// Optional longer-form source documentation from outer rustdoc comments.
    pub docs: Option<&'static str>,
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

    /// Returns one exported transition site by its stable machine-local index.
    pub fn transition(&self, index: usize) -> Option<&CodebaseTransition> {
        self.transitions.get(index)
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

    fn summary_node_id(&self) -> String {
        format!("m{}_g", self.index)
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
    /// Optional longer-form source documentation from outer rustdoc comments.
    pub docs: Option<&'static str>,
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
    /// Compiler-resolved source type identity for this validator impl.
    #[doc(hidden)]
    #[serde(skip_serializing)]
    pub resolved_source_type_name: &'static str,
    /// Optional longer-form source documentation from outer rustdoc comments.
    pub docs: Option<&'static str>,
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
    /// Optional longer-form source documentation from outer rustdoc comments.
    pub docs: Option<&'static str>,
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
pub enum CodebaseRelationKind {
    StatePayload,
    MachineField,
    TransitionParam,
}

impl CodebaseRelationKind {
    /// Human-facing kind label for relation summaries and inspector details.
    pub const fn display_label(self) -> &'static str {
        match self {
            Self::StatePayload => "payload",
            Self::MachineField => "field",
            Self::TransitionParam => "param",
        }
    }
}

/// Why one exact relation was inferred.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
pub enum CodebaseRelationBasis {
    DirectTypeSyntax,
    AttestedTypeSyntax,
    DeclaredReferenceType,
    ViaDeclaration,
}

impl CodebaseRelationBasis {
    /// Human-facing basis label for relation summaries and inspector details.
    pub const fn display_label(self) -> &'static str {
        match self {
            Self::DirectTypeSyntax => "direct type",
            Self::AttestedTypeSyntax => "attested type",
            Self::DeclaredReferenceType => "declared ref",
            Self::ViaDeclaration => "via declaration",
        }
    }

    fn summary_suffix(self) -> &'static str {
        match self {
            Self::DirectTypeSyntax => "",
            Self::AttestedTypeSyntax => " [attested]",
            Self::DeclaredReferenceType => " [ref]",
            Self::ViaDeclaration => " [via]",
        }
    }
}

/// Higher-level exact semantics for one relation after machine-role
/// classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodebaseRelationSemantic {
    Exact,
    CompositionDirectChild,
    CompositionDetachedHandoff,
}

impl CodebaseRelationSemantic {
    /// Human-facing semantic label for relation detail and search.
    pub const fn display_label(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::CompositionDirectChild => "composition direct child",
            Self::CompositionDetachedHandoff => "composition detached handoff",
        }
    }

    /// Whether this exact relation comes from direct child-machine composition.
    pub const fn is_composition_owned(self) -> bool {
        matches!(
            self,
            Self::CompositionDirectChild | Self::CompositionDetachedHandoff
        )
    }
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

impl CodebaseRelationSource {
    /// Stable source machine index for this exact relation source.
    pub const fn machine(self) -> usize {
        match self {
            Self::StatePayload { machine, .. }
            | Self::MachineField { machine, .. }
            | Self::TransitionParam { machine, .. } => machine,
        }
    }

    /// Stable source state index when the relation source is state-local.
    pub const fn state(self) -> Option<usize> {
        match self {
            Self::StatePayload { state, .. } => Some(state),
            Self::MachineField { .. } | Self::TransitionParam { .. } => None,
        }
    }

    /// Stable source transition index when the relation source is one
    /// transition parameter.
    pub const fn transition(self) -> Option<usize> {
        match self {
            Self::TransitionParam { transition, .. } => Some(transition),
            Self::StatePayload { .. } | Self::MachineField { .. } => None,
        }
    }
}

/// One resolved exact relation in the codebase export surface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CodebaseRelation {
    /// Stable codebase-local relation index.
    pub index: usize,
    /// Exact relation kind.
    pub kind: CodebaseRelationKind,
    /// Why Statum considered this relation exact.
    pub basis: CodebaseRelationBasis,
    /// Higher-level exact semantics after machine-role classification.
    pub semantic: CodebaseRelationSemantic,
    /// Exact source location for this relation.
    pub source: CodebaseRelationSource,
    /// Resolved target machine index.
    pub target_machine: usize,
    /// Resolved target state index.
    pub target_state: usize,
    /// Declared nominal reference type when this relation came through
    /// `#[machine_ref(...)]`.
    pub declared_reference_type: Option<&'static str>,
    /// Exact attested producer route when this relation came from
    /// `#[via(...)]` or a canonical `statum::Attested<_, Route>` wrapper.
    pub attested_via: Option<CodebaseAttestedRoute>,
}

impl CodebaseRelation {
    /// Stable source machine index for this exact relation.
    pub const fn source_machine(&self) -> usize {
        self.source.machine()
    }

    /// Stable source state index when this relation is state-local.
    pub const fn source_state(&self) -> Option<usize> {
        self.source.state()
    }

    /// Stable source transition index when this relation comes from one
    /// transition parameter.
    pub const fn source_transition(&self) -> Option<usize> {
        self.source.transition()
    }

    /// Stable target transition index.
    ///
    /// The current exact relation substrate does not target transitions, so
    /// this always returns `None`. The method exists so downstream navigation
    /// can keep one consistent source/target API shape.
    pub const fn target_transition(&self) -> Option<usize> {
        None
    }

    /// Whether this exact relation is owned by one composition machine through
    /// direct child-machine composition.
    pub const fn is_composition_owned(&self) -> bool {
        self.semantic.is_composition_owned()
    }
}

/// One exact producer transition reachable through an attested route.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CodebaseAttestedProducer {
    /// Stable producer machine index.
    pub machine: usize,
    /// Stable producer source-state index.
    pub state: usize,
    /// Stable producer transition index.
    pub transition: usize,
}

/// One resolved producer route attached to one exact attested relation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CodebaseAttestedRoute {
    /// Machine-module path that owns the attested route namespace.
    pub via_module_path: &'static str,
    /// Human-facing route name such as `Capture`.
    pub route_name: &'static str,
    /// Exact producer transitions that can attest this route and still satisfy
    /// the resolved consumer target state.
    pub producers: Vec<CodebaseAttestedProducer>,
}

/// One grouped machine-to-machine view derived from exact relations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CodebaseMachineRelationGroup {
    /// Stable group index in `(from_machine, to_machine)` order.
    pub index: usize,
    /// Source machine index shared by the grouped relations.
    pub from_machine: usize,
    /// Target machine index shared by the grouped relations.
    pub to_machine: usize,
    /// Higher-level group semantics derived from the grouped exact relations.
    pub semantic: CodebaseMachineRelationGroupSemantic,
    /// Stable exact relation indices included in this group.
    pub relation_indices: Vec<usize>,
    /// Stable grouped counts by relation kind and basis.
    pub counts: Vec<CodebaseRelationCount>,
}

/// Higher-level exact semantics for one grouped machine relation summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodebaseMachineRelationGroupSemantic {
    Exact,
    CompositionDirectChild,
    Mixed,
}

impl CodebaseMachineRelationGroupSemantic {
    const fn from_relation_counts(
        composition_owned_relations: usize,
        total_relations: usize,
    ) -> Self {
        if composition_owned_relations == 0 {
            Self::Exact
        } else if composition_owned_relations == total_relations {
            Self::CompositionDirectChild
        } else {
            Self::Mixed
        }
    }

    /// Human-facing semantic label for grouped relation detail.
    pub const fn display_label(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::CompositionDirectChild => "composition-owned",
            Self::Mixed => "composition + exact",
        }
    }

    const fn summary_prefix(self) -> &'static str {
        match self {
            Self::Exact => "exact refs",
            Self::CompositionDirectChild => "composition refs",
            Self::Mixed => "composition + exact refs",
        }
    }

    /// Whether this group includes any composition-owned exact relations.
    pub const fn is_composition_owned(self) -> bool {
        !matches!(self, Self::Exact)
    }
}

impl CodebaseMachineRelationGroup {
    /// Human-facing label used by machine summary edges in text renderers.
    pub fn display_label(&self) -> String {
        let counts = self
            .counts
            .iter()
            .map(CodebaseRelationCount::display_label)
            .collect::<Vec<_>>()
            .join(", ");
        format!("{}: {counts}", self.semantic.summary_prefix())
    }

    /// Whether this grouped summary includes composition-owned exact
    /// relations.
    pub const fn is_composition_owned(&self) -> bool {
        self.semantic.is_composition_owned()
    }
}

/// One grouped count inside a machine-to-machine exact relation summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
pub struct CodebaseRelationCount {
    /// Exact relation kind for this grouped count.
    pub kind: CodebaseRelationKind,
    /// Exact relation basis for this grouped count.
    pub basis: CodebaseRelationBasis,
    /// Number of exact relations in this `(kind, basis)` class.
    pub count: usize,
}

impl CodebaseRelationCount {
    /// Human-facing grouped-count label used by machine summary edges.
    pub fn display_label(&self) -> String {
        let label = format!(
            "{}{}",
            self.kind.display_label(),
            self.basis.summary_suffix()
        );
        if self.count == 1 {
            label
        } else {
            format!("{label} x{}", self.count)
        }
    }
}

/// One typed resolved view of an exact relation for downstream consumers.
#[derive(Clone, Copy, Debug)]
pub struct CodebaseAttestedProducerDetail<'a> {
    /// The exact producer record itself.
    pub producer: &'a CodebaseAttestedProducer,
    /// The resolved producer machine.
    pub machine: &'a CodebaseMachine,
    /// The resolved producer source state.
    pub state: &'a CodebaseState,
    /// The resolved producer transition.
    pub transition: &'a CodebaseTransition,
}

#[derive(Debug)]
pub struct CodebaseRelationDetail<'a> {
    /// The exact relation record itself.
    pub relation: &'a CodebaseRelation,
    /// The resolved source machine.
    pub source_machine: &'a CodebaseMachine,
    /// The resolved source state when the relation is state-local.
    pub source_state: Option<&'a CodebaseState>,
    /// The resolved source transition when the relation comes from one
    /// transition parameter.
    pub source_transition: Option<&'a CodebaseTransition>,
    /// The resolved target machine.
    pub target_machine: &'a CodebaseMachine,
    /// The resolved target state.
    pub target_state: &'a CodebaseState,
    /// The resolved producer machine when this exact relation came from one
    /// attested route declaration.
    pub attested_via_machine: Option<&'a CodebaseMachine>,
    /// The resolved producer source state when this exact relation came from
    /// one attested route declaration.
    pub attested_via_state: Option<&'a CodebaseState>,
    /// The resolved producer transition when this exact relation came from one
    /// attested route declaration and exactly one producer matched.
    pub attested_via_transition: Option<&'a CodebaseTransition>,
    /// All resolved producer transitions when this exact relation came from an
    /// attested route declaration.
    pub attested_via_producers: Vec<CodebaseAttestedProducerDetail<'a>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CodebaseRelationIndex {
    outbound_machine: Vec<Vec<usize>>,
    inbound_machine: Vec<Vec<usize>>,
    outbound_state: Vec<Vec<Vec<usize>>>,
    inbound_state: Vec<Vec<Vec<usize>>>,
    outbound_transition: Vec<Vec<Vec<usize>>>,
    inbound_transition: Vec<Vec<Vec<usize>>>,
}

impl CodebaseRelationIndex {
    fn new(machines: &[CodebaseMachine], relations: &[CodebaseRelation]) -> Self {
        let machine_count = machines.len();
        let mut index = Self {
            outbound_machine: vec![Vec::new(); machine_count],
            inbound_machine: vec![Vec::new(); machine_count],
            outbound_state: machines
                .iter()
                .map(|machine| vec![Vec::new(); machine.states.len()])
                .collect(),
            inbound_state: machines
                .iter()
                .map(|machine| vec![Vec::new(); machine.states.len()])
                .collect(),
            outbound_transition: machines
                .iter()
                .map(|machine| vec![Vec::new(); machine.transitions.len()])
                .collect(),
            inbound_transition: machines
                .iter()
                .map(|machine| vec![Vec::new(); machine.transitions.len()])
                .collect(),
        };

        for (position, relation) in relations.iter().enumerate() {
            debug_assert_eq!(relation.index, position);

            index.outbound_machine[relation.source_machine()].push(position);
            index.inbound_machine[relation.target_machine].push(position);
            if let Some(state) = relation.source_state() {
                index.outbound_state[relation.source_machine()][state].push(position);
            }
            index.inbound_state[relation.target_machine][relation.target_state].push(position);
            if let Some(transition) = relation.source_transition() {
                index.outbound_transition[relation.source_machine()][transition].push(position);
            }
            if let Some(transition) = relation.target_transition() {
                index.inbound_transition[relation.target_machine][transition].push(position);
            }
        }

        index
    }

    fn outbound_machine(&self, machine_index: usize) -> &[usize] {
        self.outbound_machine
            .get(machine_index)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn inbound_machine(&self, machine_index: usize) -> &[usize] {
        self.inbound_machine
            .get(machine_index)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn outbound_state(&self, machine_index: usize, state_index: usize) -> &[usize] {
        self.outbound_state
            .get(machine_index)
            .and_then(|states| states.get(state_index))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn inbound_state(&self, machine_index: usize, state_index: usize) -> &[usize] {
        self.inbound_state
            .get(machine_index)
            .and_then(|states| states.get(state_index))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn outbound_transition(&self, machine_index: usize, transition_index: usize) -> &[usize] {
        self.outbound_transition
            .get(machine_index)
            .and_then(|transitions| transitions.get(transition_index))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn inbound_transition(&self, machine_index: usize, transition_index: usize) -> &[usize] {
        self.inbound_transition
            .get(machine_index)
            .and_then(|transitions| transitions.get(transition_index))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

fn build_machine_relation_groups(
    relations: &[CodebaseRelation],
) -> Vec<CodebaseMachineRelationGroup> {
    let mut groups = BTreeMap::<(usize, usize), Vec<usize>>::new();
    for (position, relation) in relations.iter().enumerate() {
        debug_assert_eq!(relation.index, position);
        groups
            .entry((relation.source_machine(), relation.target_machine))
            .or_default()
            .push(position);
    }

    groups
        .into_iter()
        .enumerate()
        .map(|(index, ((from_machine, to_machine), relation_indices))| {
            let mut counts =
                BTreeMap::<(CodebaseRelationKind, CodebaseRelationBasis), usize>::new();
            let mut composition_owned_relations = 0usize;
            for relation_index in &relation_indices {
                let relation = &relations[*relation_index];
                *counts.entry((relation.kind, relation.basis)).or_default() += 1;
                if relation.is_composition_owned() {
                    composition_owned_relations += 1;
                }
            }

            CodebaseMachineRelationGroup {
                index,
                from_machine,
                to_machine,
                semantic: CodebaseMachineRelationGroupSemantic::from_relation_counts(
                    composition_owned_relations,
                    relation_indices.len(),
                ),
                relation_indices,
                counts: counts
                    .into_iter()
                    .map(|((kind, basis), count)| CodebaseRelationCount { kind, basis, count })
                    .collect(),
            }
        })
        .collect()
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
    /// One attested producer route appears more than once in the linked
    /// inventory.
    DuplicateViaRoute {
        via_module_path: &'static str,
        route_name: &'static str,
    },
    /// One linked attested producer route reuses the same route identity with a
    /// different target state.
    ConflictingViaRouteTarget {
        via_module_path: &'static str,
        route_name: &'static str,
        expected_target_state: &'static str,
        conflicting_target_state: &'static str,
    },
    /// One `#[via(...)]` exact relation points at a producer route missing from
    /// the linked inventory.
    MissingRelationViaRoute {
        relation: String,
        via_module_path: &'static str,
        route_name: &'static str,
    },
    /// One `#[via(...)]` relation points at a producer source state missing
    /// from the resolved machine graph.
    MissingRelationViaSourceState {
        machine: &'static str,
        state: &'static str,
        relation: String,
    },
    /// One `#[via(...)]` relation points at a producer transition missing from
    /// the resolved machine graph.
    MissingRelationViaTransition {
        machine: &'static str,
        state: &'static str,
        transition: &'static str,
        relation: String,
    },
    /// One attested producer route points at a target state missing from the
    /// resolved machine graph.
    MissingRelationViaTargetState {
        machine: &'static str,
        state: &'static str,
        relation: String,
    },
    /// One `#[via(...)]` relation declared an inner target state that does not
    /// match the attested producer route it references.
    MismatchedRelationViaTarget {
        relation: String,
        via_module_path: &'static str,
        route_name: &'static str,
        declared_target_state: &'static str,
        producer_target_state: &'static str,
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
            Self::DuplicateViaRoute {
                via_module_path,
                route_name,
            } => write!(
                formatter,
                "linked attested route `{via_module_path}::{route_name}` appears more than once in the producer inventory"
            ),
            Self::ConflictingViaRouteTarget {
                via_module_path,
                route_name,
                expected_target_state,
                conflicting_target_state,
            } => write!(
                formatter,
                "linked attested route `{via_module_path}::{route_name}` conflicts on target state: expected `{expected_target_state}`, found `{conflicting_target_state}`"
            ),
            Self::MissingRelationViaRoute {
                relation,
                via_module_path,
                route_name,
            } => write!(
                formatter,
                "linked exact relation `{relation}` points at missing attested route `{via_module_path}::{route_name}`"
            ),
            Self::MissingRelationViaSourceState {
                machine,
                state,
                relation,
            } => write!(
                formatter,
                "linked exact relation `{relation}` points at missing attested source state `{machine}::{state}`"
            ),
            Self::MissingRelationViaTransition {
                machine,
                state,
                transition,
                relation,
            } => write!(
                formatter,
                "linked exact relation `{relation}` points at missing attested producer transition `{machine}::{state}::{transition}`"
            ),
            Self::MissingRelationViaTargetState {
                machine,
                state,
                relation,
            } => write!(
                formatter,
                "linked exact relation `{relation}` points at missing attested target state `{machine}::{state}`"
            ),
            Self::MismatchedRelationViaTarget {
                relation,
                via_module_path,
                route_name,
                declared_target_state,
                producer_target_state,
            } => write!(
                formatter,
                "linked exact relation `{relation}` declares target state `{declared_target_state}`, but attested route `{via_module_path}::{route_name}` produces `{producer_target_state}`"
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
            docs: state.docs,
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
            docs: transition.docs,
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
            role: linked.machine.role.into(),
            label: linked.label,
            description: linked.description,
            docs: linked.docs,
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

#[derive(Clone, Copy)]
struct ResolvedViaProducer {
    machine: usize,
    state: usize,
    transition: usize,
    target_state_name: &'static str,
}

#[derive(Clone)]
struct ResolvedViaRoute {
    via_module_path: &'static str,
    route_name: &'static str,
    target_machine: usize,
    target_state: usize,
    target_state_name: &'static str,
    producers: Vec<ResolvedViaProducer>,
}

fn resolve_relations(
    machines: &[CodebaseMachine],
    relations: &'static [LinkedRelationDescriptor],
    via_routes: &'static [LinkedViaRouteDescriptor],
    reference_types: &'static [LinkedReferenceTypeDescriptor],
) -> Result<Vec<CodebaseRelation>, CodebaseDocError> {
    let reference_types = resolve_reference_type_targets(machines, reference_types)?;
    let via_routes = resolve_via_routes(machines, via_routes)?;
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

        let (resolved_target, attested_via) = match relation.target {
            LinkedRelationTarget::DirectMachine {
                machine_path,
                resolved_machine_type_name,
                state,
            } => (
                resolve_optional_target_machine(
                    machines,
                    resolved_machine_type_name(),
                    machine_path,
                    state,
                    &relation_summary(&relation),
                )?,
                None,
            ),
            LinkedRelationTarget::DeclaredReferenceType { resolved_type_name } => (
                reference_types
                    .get(resolved_type_name())
                    .copied()
                    .map(|target| {
                        (
                            target.target_machine,
                            target.target_state,
                            Some(target.rust_type_path),
                        )
                    }),
                None,
            ),
            LinkedRelationTarget::AttestedProducerRoute {
                via_module_path,
                route_name,
                resolved_route_type_name,
                route_id: _,
            } => {
                let relation_label = relation_summary(&relation);
                let resolved_route =
                    via_routes.get(resolved_route_type_name()).ok_or_else(|| {
                        CodebaseDocError::MissingRelationViaRoute {
                            relation: relation_label.clone(),
                            via_module_path,
                            route_name,
                        }
                    })?;
                (
                    Some((
                        resolved_route.target_machine,
                        resolved_route.target_state,
                        None,
                    )),
                    Some(CodebaseAttestedRoute {
                        via_module_path: resolved_route.via_module_path,
                        route_name: resolved_route.route_name,
                        producers: resolved_route
                            .producers
                            .iter()
                            .map(|producer| CodebaseAttestedProducer {
                                machine: producer.machine,
                                state: producer.state,
                                transition: producer.transition,
                            })
                            .collect(),
                    }),
                )
            }
            LinkedRelationTarget::AttestedRoute {
                via_module_path,
                route_name,
                resolved_route_type_name,
                route_id: _,
                machine_path,
                resolved_machine_type_name,
                state,
            } => {
                let relation_label = relation_summary(&relation);
                let resolved_target = resolve_optional_target_machine(
                    machines,
                    resolved_machine_type_name(),
                    machine_path,
                    state,
                    &relation_label,
                )?;
                let resolved_route =
                    via_routes.get(resolved_route_type_name()).ok_or_else(|| {
                        CodebaseDocError::MissingRelationViaRoute {
                            relation: relation_label.clone(),
                            via_module_path,
                            route_name,
                        }
                    })?;
                let matched_producers = resolved_route
                    .producers
                    .iter()
                    .filter(|producer| producer.target_state_name == state)
                    .copied()
                    .collect::<Vec<_>>();
                if matched_producers.is_empty() {
                    return Err(CodebaseDocError::MismatchedRelationViaTarget {
                        relation: relation_label,
                        via_module_path,
                        route_name,
                        declared_target_state: state,
                        producer_target_state: resolved_route
                            .producers
                            .first()
                            .map(|producer| producer.target_state_name)
                            .unwrap_or("<missing-producer-target>"),
                    });
                }
                (
                    resolved_target,
                    Some(CodebaseAttestedRoute {
                        via_module_path,
                        route_name,
                        producers: matched_producers
                            .into_iter()
                            .map(|producer| CodebaseAttestedProducer {
                                machine: producer.machine,
                                state: producer.state,
                                transition: producer.transition,
                            })
                            .collect(),
                    }),
                )
            }
        };
        let Some((target_machine, target_state, declared_reference_type)) = resolved_target else {
            continue;
        };
        let basis = map_relation_basis(relation.basis);
        let semantic = classify_relation_semantic(
            machine.role,
            basis,
            machine.index,
            target_machine,
            relation.target,
        );

        exported.push(CodebaseRelation {
            index: exported.len(),
            kind: map_relation_kind(relation.kind),
            basis,
            semantic,
            source,
            target_machine,
            target_state,
            declared_reference_type,
            attested_via,
        });
    }

    Ok(exported)
}

fn resolve_via_routes(
    machines: &[CodebaseMachine],
    via_routes: &'static [LinkedViaRouteDescriptor],
) -> Result<HashMap<&'static str, ResolvedViaRoute>, CodebaseDocError> {
    let exact_machine_positions = machines
        .iter()
        .map(|machine| (machine.rust_type_path, machine.index))
        .collect::<HashMap<_, _>>();
    let mut resolved = HashMap::with_capacity(via_routes.len());

    for route in via_routes {
        let machine_index = resolve_relation_source_machine(
            machines,
            &exact_machine_positions,
            route.machine.rust_type_path,
            route.route_name,
        )?;
        let machine = &machines[machine_index];
        let state = machine
            .state_named(route.source_state)
            .map(|state| state.index)
            .ok_or_else(|| CodebaseDocError::MissingRelationViaSourceState {
                machine: machine.rust_type_path,
                state: route.source_state,
                relation: format!("{}::{}", route.via_module_path, route.route_name),
            })?;
        let transition = machine
            .transition_site(route.source_state, route.transition)
            .map(|transition| transition.index)
            .ok_or_else(|| CodebaseDocError::MissingRelationViaTransition {
                machine: machine.rust_type_path,
                state: route.source_state,
                transition: route.transition,
                relation: format!("{}::{}", route.via_module_path, route.route_name),
            })?;
        let target_state = machine
            .state_named(route.target_state)
            .map(|state| state.index)
            .ok_or_else(|| CodebaseDocError::MissingRelationViaTargetState {
                machine: machine.rust_type_path,
                state: route.target_state,
                relation: format!("{}::{}", route.via_module_path, route.route_name),
            })?;
        let producer = ResolvedViaProducer {
            machine: machine.index,
            state,
            transition,
            target_state_name: route.target_state,
        };
        let resolved_route_type_name = (route.resolved_route_type_name)();
        match resolved.entry(resolved_route_type_name) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(ResolvedViaRoute {
                    via_module_path: route.via_module_path,
                    route_name: route.route_name,
                    target_machine: machine.index,
                    target_state,
                    target_state_name: route.target_state,
                    producers: vec![producer],
                });
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let resolved_route = entry.get_mut();
                if resolved_route.via_module_path != route.via_module_path
                    || resolved_route.route_name != route.route_name
                {
                    return Err(CodebaseDocError::DuplicateViaRoute {
                        via_module_path: route.via_module_path,
                        route_name: route.route_name,
                    });
                }
                if resolved_route.target_machine != machine.index {
                    return Err(CodebaseDocError::DuplicateViaRoute {
                        via_module_path: route.via_module_path,
                        route_name: route.route_name,
                    });
                }
                if resolved_route.target_state != target_state {
                    return Err(CodebaseDocError::ConflictingViaRouteTarget {
                        via_module_path: route.via_module_path,
                        route_name: route.route_name,
                        expected_target_state: resolved_route.target_state_name,
                        conflicting_target_state: route.target_state,
                    });
                }
                if resolved_route.producers.iter().any(|existing| {
                    existing.machine == producer.machine
                        && existing.state == producer.state
                        && existing.transition == producer.transition
                        && existing.target_state_name == producer.target_state_name
                }) {
                    return Err(CodebaseDocError::DuplicateViaRoute {
                        via_module_path: route.via_module_path,
                        route_name: route.route_name,
                    });
                }
                resolved_route.producers.push(producer);
                resolved_route.producers.sort_by(|left, right| {
                    left.machine
                        .cmp(&right.machine)
                        .then(left.state.cmp(&right.state))
                        .then(left.transition.cmp(&right.transition))
                        .then(left.target_state_name.cmp(right.target_state_name))
                });
            }
        }
    }

    Ok(resolved)
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
            (reference_type.resolved_target_machine_type_name)(),
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
    resolved_machine_type_name: &str,
    machine_path: &'static [&'static str],
    state: &'static str,
    relation: &str,
) -> Result<Option<ResolvedRelationTarget>, CodebaseDocError> {
    let candidates = target_candidates(machines, resolved_machine_type_name, machine_path, state);
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
    resolved_machine_type_name: &str,
    machine_path: &'static [&'static str],
    state: &'static str,
    missing_machine: FMissingMachine,
    missing_state: FMissingState,
    ambiguous: FAmbiguous,
) -> Result<(usize, usize), CodebaseDocError>
where
    FMissingMachine: FnOnce(String, &'static str) -> CodebaseDocError,
    FMissingState: FnOnce(String, &'static str) -> CodebaseDocError,
    FAmbiguous: FnOnce(String, &'static str) -> CodebaseDocError,
{
    let machine_path_string = machine_path.join("::");
    let matching_machines = machines
        .iter()
        .filter(|candidate| {
            machine_path_matches(
                candidate.rust_type_path,
                resolved_machine_type_name,
                machine_path,
            )
        })
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
    resolved_machine_type_name: &str,
    machine_path: &'static [&'static str],
    state: &'static str,
) -> Vec<(usize, usize)> {
    machines
        .iter()
        .filter_map(|candidate| {
            if !machine_path_matches(
                candidate.rust_type_path,
                resolved_machine_type_name,
                machine_path,
            ) {
                return None;
            }

            candidate
                .state_named(state)
                .map(|target_state| (candidate.index, target_state.index))
        })
        .collect()
}

fn machine_path_matches(
    candidate: &str,
    resolved_machine_type_name: &str,
    path: &[&'static str],
) -> bool {
    machine_family_path_suffix_matches(resolved_machine_type_name, candidate)
        || path_suffix_matches(candidate, path)
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
        LinkedRelationBasis::AttestedTypeSyntax => CodebaseRelationBasis::AttestedTypeSyntax,
        LinkedRelationBasis::DeclaredReferenceType => CodebaseRelationBasis::DeclaredReferenceType,
        LinkedRelationBasis::ViaDeclaration => CodebaseRelationBasis::ViaDeclaration,
    }
}

fn classify_relation_semantic(
    source_role: CodebaseMachineRole,
    basis: CodebaseRelationBasis,
    source_machine: usize,
    target_machine: usize,
    target: LinkedRelationTarget,
) -> CodebaseRelationSemantic {
    if source_role == CodebaseMachineRole::Composition && source_machine != target_machine {
        match (basis, target) {
            (
                CodebaseRelationBasis::DirectTypeSyntax,
                LinkedRelationTarget::DirectMachine { .. },
            ) => CodebaseRelationSemantic::CompositionDirectChild,
            (
                CodebaseRelationBasis::AttestedTypeSyntax | CodebaseRelationBasis::ViaDeclaration,
                LinkedRelationTarget::AttestedProducerRoute { .. },
            ) => CodebaseRelationSemantic::CompositionDetachedHandoff,
            _ => CodebaseRelationSemantic::Exact,
        }
    } else {
        CodebaseRelationSemantic::Exact
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
        .then_with(|| {
            (left.resolved_target_machine_type_name)()
                .cmp((right.resolved_target_machine_type_name)())
        })
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
                resolved_machine_type_name: left_type_name,
                state: left_state,
            },
            LinkedRelationTarget::DirectMachine {
                machine_path: right_path,
                resolved_machine_type_name: right_type_name,
                state: right_state,
            },
        ) => left_path
            .cmp(right_path)
            .then_with(|| left_type_name().cmp(right_type_name()))
            .then_with(|| left_state.cmp(right_state)),
        (
            LinkedRelationTarget::DeclaredReferenceType {
                resolved_type_name: left_name,
            },
            LinkedRelationTarget::DeclaredReferenceType {
                resolved_type_name: right_name,
            },
        ) => left_name().cmp(right_name()),
        (
            LinkedRelationTarget::AttestedProducerRoute {
                via_module_path: left_module_path,
                route_name: left_route_name,
                resolved_route_type_name: left_type_name,
                route_id: left_route_id,
            },
            LinkedRelationTarget::AttestedProducerRoute {
                via_module_path: right_module_path,
                route_name: right_route_name,
                resolved_route_type_name: right_type_name,
                route_id: right_route_id,
            },
        ) => left_module_path
            .cmp(right_module_path)
            .then_with(|| left_route_name.cmp(right_route_name))
            .then_with(|| left_type_name().cmp(right_type_name()))
            .then_with(|| left_route_id.cmp(right_route_id)),
        (
            LinkedRelationTarget::AttestedRoute {
                via_module_path: left_module_path,
                route_name: left_route_name,
                resolved_route_type_name: left_type_name,
                route_id: left_route_id,
                machine_path: left_machine_path,
                resolved_machine_type_name: left_machine_type_name,
                state: left_state,
            },
            LinkedRelationTarget::AttestedRoute {
                via_module_path: right_module_path,
                route_name: right_route_name,
                resolved_route_type_name: right_type_name,
                route_id: right_route_id,
                machine_path: right_machine_path,
                resolved_machine_type_name: right_machine_type_name,
                state: right_state,
            },
        ) => left_module_path
            .cmp(right_module_path)
            .then_with(|| left_route_name.cmp(right_route_name))
            .then_with(|| left_type_name().cmp(right_type_name()))
            .then_with(|| left_route_id.cmp(right_route_id))
            .then_with(|| left_machine_path.cmp(right_machine_path))
            .then_with(|| left_machine_type_name().cmp(right_machine_type_name()))
            .then_with(|| left_state.cmp(right_state)),
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
        LinkedRelationBasis::AttestedTypeSyntax => 1,
        LinkedRelationBasis::DeclaredReferenceType => 2,
        LinkedRelationBasis::ViaDeclaration => 3,
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
        LinkedRelationTarget::AttestedProducerRoute { .. } => 2,
        LinkedRelationTarget::AttestedRoute { .. } => 3,
    }
}

fn relation_summary(relation: &LinkedRelationDescriptor) -> String {
    let base = match relation.source {
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
    };

    match relation.target {
        LinkedRelationTarget::AttestedProducerRoute {
            via_module_path,
            route_name,
            ..
        }
        | LinkedRelationTarget::AttestedRoute {
            via_module_path,
            route_name,
            ..
        } => format!("{base} via {via_module_path}::{route_name}"),
        LinkedRelationTarget::DirectMachine { .. }
        | LinkedRelationTarget::DeclaredReferenceType { .. } => base,
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
            resolved_source_type_name: (entry.resolved_source_type_name)(),
            docs: entry.docs,
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

fn machine_family_path_suffix_matches(
    resolved_machine_type_name: &str,
    machine_path: &str,
) -> bool {
    let family_path = resolved_machine_type_name
        .split_once('<')
        .map(|(family_path, _)| family_path)
        .unwrap_or(resolved_machine_type_name);
    path_string_suffix_matches(family_path, machine_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    use statum::{
        LinkedMachineGraph, LinkedStateDescriptor, LinkedTransitionDescriptor,
        LinkedTransitionInventory, MachineDescriptor,
    };

    static PRODUCER_STATES: [LinkedStateDescriptor; 2] = [
        LinkedStateDescriptor {
            rust_name: "Authorized",
            label: None,
            description: None,
            docs: None,
            has_data: false,
            direct_construction_available: false,
        },
        LinkedStateDescriptor {
            rust_name: "Captured",
            label: None,
            description: None,
            docs: None,
            has_data: false,
            direct_construction_available: false,
        },
    ];
    static PRODUCER_TRANSITIONS: [LinkedTransitionDescriptor; 1] = [LinkedTransitionDescriptor {
        method_name: "capture",
        label: None,
        description: None,
        docs: None,
        from: "Authorized",
        to: &["Captured"],
    }];
    static PRODUCER_MACHINE: LinkedMachineGraph = LinkedMachineGraph {
        machine: MachineDescriptor {
            module_path: "crate::payment",
            rust_type_path: "crate::payment::Machine",
            role: statum::MachineRole::Protocol,
        },
        label: None,
        description: None,
        docs: None,
        states: &PRODUCER_STATES,
        transitions: LinkedTransitionInventory::new(producer_transitions),
        static_links: &[],
    };

    static CONSUMER_STATES: [LinkedStateDescriptor; 2] = [
        LinkedStateDescriptor {
            rust_name: "Draft",
            label: None,
            description: None,
            docs: None,
            has_data: false,
            direct_construction_available: false,
        },
        LinkedStateDescriptor {
            rust_name: "Done",
            label: None,
            description: None,
            docs: None,
            has_data: false,
            direct_construction_available: false,
        },
    ];
    static CONSUMER_TRANSITIONS: [LinkedTransitionDescriptor; 1] = [LinkedTransitionDescriptor {
        method_name: "finish",
        label: None,
        description: None,
        docs: None,
        from: "Draft",
        to: &["Done"],
    }];
    static CONSUMER_MACHINE: LinkedMachineGraph = LinkedMachineGraph {
        machine: MachineDescriptor {
            module_path: "crate::audit",
            rust_type_path: "crate::audit::Machine",
            role: statum::MachineRole::Protocol,
        },
        label: None,
        description: None,
        docs: None,
        states: &CONSUMER_STATES,
        transitions: LinkedTransitionInventory::new(consumer_transitions),
        static_links: &[],
    };

    static LINKED_MACHINES: [LinkedMachineGraph; 2] = [PRODUCER_MACHINE, CONSUMER_MACHINE];
    static CONFLICTING_VIA_ROUTES: [LinkedViaRouteDescriptor; 2] = [
        LinkedViaRouteDescriptor {
            machine: MachineDescriptor {
                module_path: "crate::payment",
                rust_type_path: "crate::payment::Machine",
                role: statum::MachineRole::Protocol,
            },
            via_module_path: "crate::payment::via",
            route_name: "Capture",
            resolved_route_type_name: capture_route_type_name,
            route_id: 1,
            transition: "capture",
            source_state: "Authorized",
            target_state: "Captured",
        },
        LinkedViaRouteDescriptor {
            machine: MachineDescriptor {
                module_path: "crate::payment",
                rust_type_path: "crate::payment::Machine",
                role: statum::MachineRole::Protocol,
            },
            via_module_path: "crate::receipts::via",
            route_name: "Release",
            resolved_route_type_name: capture_route_type_name,
            route_id: 2,
            transition: "capture",
            source_state: "Authorized",
            target_state: "Captured",
        },
    ];
    static CONFLICTING_VIA_ROUTE_TARGETS: [LinkedViaRouteDescriptor; 2] = [
        LinkedViaRouteDescriptor {
            machine: MachineDescriptor {
                module_path: "crate::payment",
                rust_type_path: "crate::payment::Machine",
                role: statum::MachineRole::Protocol,
            },
            via_module_path: "crate::payment::via",
            route_name: "Capture",
            resolved_route_type_name: capture_route_type_name,
            route_id: 1,
            transition: "capture",
            source_state: "Authorized",
            target_state: "Captured",
        },
        LinkedViaRouteDescriptor {
            machine: MachineDescriptor {
                module_path: "crate::payment",
                rust_type_path: "crate::payment::Machine",
                role: statum::MachineRole::Protocol,
            },
            via_module_path: "crate::payment::via",
            route_name: "Capture",
            resolved_route_type_name: capture_route_type_name,
            route_id: 1,
            transition: "capture",
            source_state: "Authorized",
            target_state: "Authorized",
        },
    ];

    #[test]
    fn conflicting_attested_route_identities_fail_closed() {
        let err = CodebaseDoc::try_from_linked_with_inventories(
            &LINKED_MACHINES,
            &[],
            &[],
            &CONFLICTING_VIA_ROUTES,
            &[],
        )
        .expect_err("conflicting route identities should fail closed");

        assert_eq!(
            err,
            CodebaseDocError::DuplicateViaRoute {
                via_module_path: "crate::receipts::via",
                route_name: "Release",
            }
        );
        assert_eq!(
            err.to_string(),
            "linked attested route `crate::receipts::via::Release` appears more than once in the producer inventory"
        );
    }

    #[test]
    fn conflicting_attested_route_targets_fail_closed() {
        let err = CodebaseDoc::try_from_linked_with_inventories(
            &LINKED_MACHINES,
            &[],
            &[],
            &CONFLICTING_VIA_ROUTE_TARGETS,
            &[],
        )
        .expect_err("conflicting route targets should fail closed");

        assert_eq!(
            err,
            CodebaseDocError::ConflictingViaRouteTarget {
                via_module_path: "crate::payment::via",
                route_name: "Capture",
                expected_target_state: "Captured",
                conflicting_target_state: "Authorized",
            }
        );
        assert_eq!(
            err.to_string(),
            "linked attested route `crate::payment::via::Capture` conflicts on target state: expected `Captured`, found `Authorized`"
        );
    }

    #[test]
    fn composition_semantics_require_cross_machine_direct_targets() {
        assert_eq!(
            classify_relation_semantic(
                CodebaseMachineRole::Composition,
                CodebaseRelationBasis::DirectTypeSyntax,
                1,
                2,
                LinkedRelationTarget::DirectMachine {
                    machine_path: &["crate", "task", "Machine"],
                    resolved_machine_type_name: capture_route_type_name,
                    state: "Running",
                },
            ),
            CodebaseRelationSemantic::CompositionDirectChild
        );
        assert_eq!(
            classify_relation_semantic(
                CodebaseMachineRole::Composition,
                CodebaseRelationBasis::DirectTypeSyntax,
                1,
                1,
                LinkedRelationTarget::DirectMachine {
                    machine_path: &["crate", "task", "Machine"],
                    resolved_machine_type_name: capture_route_type_name,
                    state: "Running",
                },
            ),
            CodebaseRelationSemantic::Exact
        );
        assert_eq!(
            classify_relation_semantic(
                CodebaseMachineRole::Composition,
                CodebaseRelationBasis::DeclaredReferenceType,
                1,
                2,
                LinkedRelationTarget::DeclaredReferenceType {
                    resolved_type_name: capture_route_type_name,
                },
            ),
            CodebaseRelationSemantic::Exact
        );
        assert_eq!(
            classify_relation_semantic(
                CodebaseMachineRole::Composition,
                CodebaseRelationBasis::AttestedTypeSyntax,
                1,
                2,
                LinkedRelationTarget::AttestedProducerRoute {
                    via_module_path: "crate::task::via",
                    route_name: "Capture",
                    resolved_route_type_name: capture_route_type_name,
                    route_id: 1,
                },
            ),
            CodebaseRelationSemantic::CompositionDetachedHandoff
        );
    }

    #[test]
    fn cached_relation_indices_follow_stable_export_order() {
        let machines = vec![
            CodebaseMachine {
                index: 0,
                module_path: "crate::producer",
                rust_type_path: "crate::producer::Machine",
                role: CodebaseMachineRole::Protocol,
                label: None,
                description: None,
                docs: None,
                states: vec![CodebaseState {
                    index: 0,
                    rust_name: "Idle",
                    label: None,
                    description: None,
                    docs: None,
                    has_data: false,
                    direct_construction_available: false,
                    is_graph_root: true,
                }],
                transitions: vec![CodebaseTransition {
                    index: 0,
                    method_name: "start",
                    label: None,
                    description: None,
                    docs: None,
                    from: 0,
                    to: vec![0],
                }],
                validator_entries: Vec::new(),
            },
            CodebaseMachine {
                index: 1,
                module_path: "crate::consumer",
                rust_type_path: "crate::consumer::Machine",
                role: CodebaseMachineRole::Composition,
                label: None,
                description: None,
                docs: None,
                states: vec![CodebaseState {
                    index: 0,
                    rust_name: "Done",
                    label: None,
                    description: None,
                    docs: None,
                    has_data: false,
                    direct_construction_available: false,
                    is_graph_root: true,
                }],
                transitions: Vec::new(),
                validator_entries: Vec::new(),
            },
        ];
        let relations = vec![
            CodebaseRelation {
                index: 0,
                kind: CodebaseRelationKind::TransitionParam,
                basis: CodebaseRelationBasis::DirectTypeSyntax,
                semantic: CodebaseRelationSemantic::CompositionDirectChild,
                source: CodebaseRelationSource::TransitionParam {
                    machine: 0,
                    transition: 0,
                    param_index: 0,
                    param_name: None,
                },
                target_machine: 1,
                target_state: 0,
                declared_reference_type: None,
                attested_via: None,
            },
            CodebaseRelation {
                index: 1,
                kind: CodebaseRelationKind::StatePayload,
                basis: CodebaseRelationBasis::DirectTypeSyntax,
                semantic: CodebaseRelationSemantic::Exact,
                source: CodebaseRelationSource::StatePayload {
                    machine: 0,
                    state: 0,
                    field_name: Some("child"),
                },
                target_machine: 1,
                target_state: 0,
                declared_reference_type: None,
                attested_via: None,
            },
        ];

        let index = CodebaseRelationIndex::new(&machines, &relations);
        let groups = build_machine_relation_groups(&relations);

        assert_eq!(index.outbound_machine(0), &[0, 1]);
        assert_eq!(index.inbound_machine(1), &[0, 1]);
        assert_eq!(groups[0].relation_indices, vec![0, 1]);
    }

    fn producer_transitions() -> &'static [LinkedTransitionDescriptor] {
        &PRODUCER_TRANSITIONS
    }

    fn consumer_transitions() -> &'static [LinkedTransitionDescriptor] {
        &CONSUMER_TRANSITIONS
    }

    fn capture_route_type_name() -> &'static str {
        "crate::payment::machine::via::Capture"
    }
}
