use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use module_path_extractor::{
    module_path_from_file_with_root, module_path_to_file, module_root_from_file,
};
use quote::ToTokens;
use statum_graph::{CodebaseDoc, CodebaseMachine, CodebaseState, CodebaseTransition};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{
    AttrStyle, FnArg, ImplItem, ImplItemFn, Item, ItemEnum, ItemImpl, ItemMod, ItemStruct,
    ReturnType, Type, TypePath, UseTree,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectPackageSource {
    pub package_name: String,
    pub manifest_dir: PathBuf,
    pub lib_target_path: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum HeuristicEvidenceKind {
    Signature,
    Body,
}

impl HeuristicEvidenceKind {
    pub const fn display_label(self) -> &'static str {
        match self {
            Self::Signature => "type surface",
            Self::Body => "body",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeuristicStatusKind {
    Available,
    Partial,
    Unavailable,
}

impl HeuristicStatusKind {
    pub const fn display_label(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Partial => "partial",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeuristicDiagnostic {
    pub context: String,
    pub message: String,
}

impl HeuristicDiagnostic {
    pub fn display_label(&self) -> String {
        if self.context.is_empty() {
            self.message.clone()
        } else {
            format!("{}: {}", self.context, self.message)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum HeuristicRelationSource {
    State {
        machine: usize,
        state: usize,
    },
    Transition {
        machine: usize,
        transition: usize,
    },
    Method {
        machine: usize,
        state: usize,
        method_name: String,
    },
}

impl HeuristicRelationSource {
    pub const fn machine(&self) -> usize {
        match *self {
            Self::State { machine, .. }
            | Self::Transition { machine, .. }
            | Self::Method { machine, .. } => machine,
        }
    }

    pub const fn state(&self) -> Option<usize> {
        match *self {
            Self::State { state, .. } | Self::Method { state, .. } => Some(state),
            Self::Transition { .. } => None,
        }
    }

    pub const fn transition(&self) -> Option<usize> {
        match *self {
            Self::Transition { transition, .. } => Some(transition),
            Self::State { .. } | Self::Method { .. } => None,
        }
    }

    pub fn method_name(&self) -> Option<&str> {
        match self {
            Self::Method { method_name, .. } => Some(method_name),
            Self::State { .. } | Self::Transition { .. } => None,
        }
    }

    pub const fn kind_label(&self) -> &'static str {
        match self {
            Self::State { .. } => "state",
            Self::Transition { .. } => "transition",
            Self::Method { .. } => "method",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeuristicRelation {
    pub index: usize,
    pub source: HeuristicRelationSource,
    pub target_machine: usize,
    pub evidence_kind: HeuristicEvidenceKind,
    pub matched_path_text: String,
    pub file_path: PathBuf,
    pub line_number: usize,
    pub snippet: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub struct HeuristicRelationDetail<'a> {
    pub relation: &'a HeuristicRelation,
    pub source_machine: &'a CodebaseMachine,
    pub source_state: Option<&'a CodebaseState>,
    pub source_transition: Option<&'a CodebaseTransition>,
    pub target_machine: &'a CodebaseMachine,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct HeuristicRelationCount {
    pub evidence_kind: HeuristicEvidenceKind,
    pub count: usize,
}

impl HeuristicRelationCount {
    pub fn display_label(&self) -> String {
        if self.count == 1 {
            self.evidence_kind.display_label().to_owned()
        } else {
            format!("{} x{}", self.evidence_kind.display_label(), self.count)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeuristicMachineRelationGroup {
    pub index: usize,
    pub from_machine: usize,
    pub to_machine: usize,
    pub relation_indices: Vec<usize>,
    pub counts: Vec<HeuristicRelationCount>,
}

impl HeuristicMachineRelationGroup {
    pub fn display_label(&self) -> String {
        let counts = self
            .counts
            .iter()
            .map(HeuristicRelationCount::display_label)
            .collect::<Vec<_>>()
            .join(", ");
        format!("heuristic refs: {counts}")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeuristicOverlay {
    status: HeuristicStatusKind,
    diagnostics: Vec<HeuristicDiagnostic>,
    relations: Vec<HeuristicRelation>,
}

impl HeuristicOverlay {
    pub fn status(&self) -> HeuristicStatusKind {
        self.status
    }

    pub fn diagnostics(&self) -> &[HeuristicDiagnostic] {
        &self.diagnostics
    }

    pub fn relations(&self) -> &[HeuristicRelation] {
        &self.relations
    }

    pub fn relation(&self, index: usize) -> Option<&HeuristicRelation> {
        self.relations.get(index)
    }

    pub fn machine_relation_groups(&self) -> Vec<HeuristicMachineRelationGroup> {
        let mut groups = BTreeMap::<(usize, usize), Vec<usize>>::new();
        for relation in &self.relations {
            groups
                .entry((relation.source.machine(), relation.target_machine))
                .or_default()
                .push(relation.index);
        }

        groups
            .into_iter()
            .enumerate()
            .map(|(index, ((from_machine, to_machine), relation_indices))| {
                let mut counts = BTreeMap::<HeuristicEvidenceKind, usize>::new();
                for relation_index in &relation_indices {
                    let relation = self
                        .relation(*relation_index)
                        .expect("grouped heuristic relation index should resolve");
                    *counts.entry(relation.evidence_kind).or_default() += 1;
                }

                HeuristicMachineRelationGroup {
                    index,
                    from_machine,
                    to_machine,
                    relation_indices,
                    counts: counts
                        .into_iter()
                        .map(|(evidence_kind, count)| HeuristicRelationCount {
                            evidence_kind,
                            count,
                        })
                        .collect(),
                }
            })
            .collect()
    }

    pub fn outbound_relations_for_machine(
        &self,
        machine_index: usize,
    ) -> impl Iterator<Item = &HeuristicRelation> + '_ {
        self.relations
            .iter()
            .filter(move |relation| relation.source.machine() == machine_index)
    }

    pub fn inbound_relations_for_machine(
        &self,
        machine_index: usize,
    ) -> impl Iterator<Item = &HeuristicRelation> + '_ {
        self.relations
            .iter()
            .filter(move |relation| relation.target_machine == machine_index)
    }

    pub fn outbound_relations_for_transition(
        &self,
        machine_index: usize,
        transition_index: usize,
    ) -> impl Iterator<Item = &HeuristicRelation> + '_ {
        self.relations.iter().filter(move |relation| {
            relation.source.machine() == machine_index
                && relation.source.transition() == Some(transition_index)
        })
    }

    pub fn outbound_relations_for_state(
        &self,
        machine_index: usize,
        state_index: usize,
    ) -> impl Iterator<Item = &HeuristicRelation> + '_ {
        self.relations.iter().filter(move |relation| {
            relation.source.machine() == machine_index
                && relation.source.state() == Some(state_index)
        })
    }

    pub fn inbound_relations_for_transition(
        &self,
        _machine_index: usize,
        _transition_index: usize,
    ) -> impl Iterator<Item = &HeuristicRelation> + '_ {
        self.relations.iter().filter(|_| false)
    }

    pub fn relation_detail<'a>(
        &'a self,
        doc: &'a CodebaseDoc,
        index: usize,
    ) -> Option<HeuristicRelationDetail<'a>> {
        let relation = self.relation(index)?;
        let source_machine = doc.machine(relation.source.machine())?;
        let source_state = relation
            .source
            .state()
            .and_then(|state_index| source_machine.state(state_index));
        let source_transition = relation
            .source
            .transition()
            .and_then(|transition_index| source_machine.transition(transition_index));
        let target_machine = doc.machine(relation.target_machine)?;

        Some(HeuristicRelationDetail {
            relation,
            source_machine,
            source_state,
            source_transition,
            target_machine,
        })
    }

    #[cfg(test)]
    pub(crate) fn from_parts(
        status: HeuristicStatusKind,
        diagnostics: Vec<HeuristicDiagnostic>,
        relations: Vec<HeuristicRelation>,
    ) -> Self {
        Self {
            status,
            diagnostics,
            relations,
        }
    }
}

pub fn collect_heuristic_overlay(
    doc: &CodebaseDoc,
    packages: &[InspectPackageSource],
) -> HeuristicOverlay {
    let inventory = MachineInventory::new(doc);
    let mut collector = OverlayCollector::new(doc, &inventory);
    for package in packages {
        collector.scan_package(package);
    }
    collector.finish()
}

type UseMap = BTreeMap<String, String>;

struct LocalTypeRegistry<'a> {
    structs: HashMap<String, &'a ItemStruct>,
}

impl<'a> LocalTypeRegistry<'a> {
    fn new(items: &'a [Item]) -> Self {
        let structs = items
            .iter()
            .filter_map(|item| match item {
                Item::Struct(item_struct) if !has_cfg_attrs(&item_struct.attrs) => {
                    Some((item_struct.ident.to_string(), item_struct))
                }
                _ => None,
            })
            .collect();
        Self { structs }
    }

    fn single_segment_struct(&self, type_path: &TypePath) -> Option<&'a ItemStruct> {
        if type_path.qself.is_some() || type_path.path.leading_colon.is_some() {
            return None;
        }
        let mut segments = type_path.path.segments.iter();
        let segment = segments.next()?;
        if segments.next().is_some() {
            return None;
        }
        self.structs.get(&segment.ident.to_string()).copied()
    }
}

struct OverlayCollector<'a> {
    doc: &'a CodebaseDoc,
    inventory: &'a MachineInventory<'a>,
    diagnostics: Vec<HeuristicDiagnostic>,
    relations: BTreeMap<HeuristicRelationKey, HeuristicRelationCandidate>,
    visited_modules: HashSet<(PathBuf, String)>,
    scanned_files: usize,
}

impl<'a> OverlayCollector<'a> {
    fn new(doc: &'a CodebaseDoc, inventory: &'a MachineInventory<'a>) -> Self {
        Self {
            doc,
            inventory,
            diagnostics: Vec::new(),
            relations: BTreeMap::new(),
            visited_modules: HashSet::new(),
            scanned_files: 0,
        }
    }

    fn scan_package(&mut self, package: &InspectPackageSource) {
        let module_root = module_root_from_file(&package.lib_target_path.to_string_lossy());
        let module_path = module_path_from_file_with_root(
            &package.lib_target_path.to_string_lossy(),
            &module_root,
        );
        self.scan_module_file(
            package,
            &module_root,
            &package.lib_target_path,
            &module_path,
        );
    }

    fn scan_module_file(
        &mut self,
        package: &InspectPackageSource,
        module_root: &Path,
        file_path: &Path,
        module_path: &str,
    ) {
        let normalized_file = normalize_absolute_path(file_path);
        if !self
            .visited_modules
            .insert((normalized_file.clone(), module_path.to_owned()))
        {
            return;
        }

        let source = match fs::read_to_string(&normalized_file) {
            Ok(source) => source,
            Err(error) => {
                self.push_diagnostic(
                    format!("package {}", package.package_name),
                    format!("failed to read `{}`: {error}", normalized_file.display()),
                );
                return;
            }
        };
        let parsed = match syn::parse_file(&source) {
            Ok(parsed) => parsed,
            Err(error) => {
                self.push_diagnostic(
                    format!("file {}", normalized_file.display()),
                    format!("failed to parse source: {error}"),
                );
                return;
            }
        };

        self.scanned_files += 1;
        self.scan_module_items(
            package,
            module_root,
            &normalized_file,
            &source,
            module_path,
            &parsed.items,
        );
    }

    fn scan_module_items(
        &mut self,
        package: &InspectPackageSource,
        module_root: &Path,
        file_path: &Path,
        source: &str,
        module_path: &str,
        items: &[Item],
    ) {
        let imports = build_use_map(items, module_path);
        let local_types = LocalTypeRegistry::new(items);
        for item in items {
            match item {
                Item::Impl(item_impl) => self.scan_impl(
                    package,
                    file_path,
                    source,
                    module_path,
                    &imports,
                    &local_types,
                    item_impl,
                ),
                Item::Enum(item_enum) => self.scan_state_enum(
                    file_path,
                    source,
                    module_path,
                    &imports,
                    &local_types,
                    item_enum,
                ),
                Item::Mod(item_mod) => self.scan_child_module(
                    package,
                    module_root,
                    file_path,
                    source,
                    module_path,
                    item_mod,
                ),
                _ => {}
            }
        }
    }

    fn scan_child_module(
        &mut self,
        package: &InspectPackageSource,
        module_root: &Path,
        file_path: &Path,
        source: &str,
        module_path: &str,
        item_mod: &ItemMod,
    ) {
        if has_cfg_attrs(&item_mod.attrs) {
            return;
        }

        let child_module_path = join_module_path(module_path, &item_mod.ident.to_string());
        if let Some((_, items)) = item_mod.content.as_ref() {
            self.scan_module_items(
                package,
                module_root,
                file_path,
                source,
                &child_module_path,
                items,
            );
            return;
        }

        let child_file_path = match explicit_module_file_path(item_mod, file_path).or_else(|| {
            module_path_to_file(
                &child_module_path,
                &file_path.to_string_lossy(),
                module_root,
            )
        }) {
            Some(child_file_path) => child_file_path,
            None => {
                self.push_diagnostic(
                    format!("module {child_module_path}"),
                    format!(
                        "could not resolve source file from `{}`",
                        file_path.display()
                    ),
                );
                return;
            }
        };
        self.scan_module_file(package, module_root, &child_file_path, &child_module_path);
    }

    #[allow(clippy::too_many_arguments)]
    fn scan_impl(
        &mut self,
        package: &InspectPackageSource,
        file_path: &Path,
        source: &str,
        module_path: &str,
        imports: &UseMap,
        local_types: &LocalTypeRegistry<'_>,
        item_impl: &ItemImpl,
    ) {
        if has_transition_attr(&item_impl.attrs) {
            self.scan_transition_impl(
                package,
                file_path,
                source,
                module_path,
                imports,
                local_types,
                item_impl,
            );
        } else {
            self.scan_method_impl(
                file_path,
                source,
                module_path,
                imports,
                local_types,
                item_impl,
            );
        }
    }

    fn scan_state_enum(
        &mut self,
        file_path: &Path,
        source: &str,
        module_path: &str,
        imports: &UseMap,
        local_types: &LocalTypeRegistry<'_>,
        item_enum: &ItemEnum,
    ) {
        if !has_state_attr(&item_enum.attrs) || has_cfg_attrs(&item_enum.attrs) {
            return;
        }

        let Some(machine) = self.inventory.resolve_machine_in_module(module_path) else {
            return;
        };

        for variant in &item_enum.variants {
            if has_cfg_attrs(&variant.attrs) {
                continue;
            }

            let Some(source_state) = machine.state_named(&variant.ident.to_string()) else {
                continue;
            };
            let relation_source = HeuristicRelationSource::State {
                machine: machine.index,
                state: source_state.index,
            };
            let context = format!("state {}::{}", machine.rust_type_path, variant.ident);
            for field in &variant.fields {
                if has_cfg_attrs(&field.attrs) {
                    continue;
                }
                self.collect_type_surface_relations(
                    file_path,
                    source,
                    module_path,
                    imports,
                    &context,
                    &relation_source,
                    &field.ty,
                    local_types,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn scan_transition_impl(
        &mut self,
        _package: &InspectPackageSource,
        file_path: &Path,
        source: &str,
        module_path: &str,
        imports: &UseMap,
        local_types: &LocalTypeRegistry<'_>,
        item_impl: &ItemImpl,
    ) {
        if has_cfg_attrs(&item_impl.attrs) {
            return;
        }

        let Some((source_machine_index, _source_state_index, source_state_name)) = self
            .inventory
            .resolve_state_impl(module_path, &item_impl.self_ty)
        else {
            return;
        };
        let source_machine = self
            .doc
            .machine(source_machine_index)
            .expect("resolved heuristic source machine should exist");

        for item in &item_impl.items {
            let ImplItem::Fn(method) = item else {
                continue;
            };
            if has_cfg_attrs(&method.attrs) {
                continue;
            }

            let method_name = method.sig.ident.to_string();
            let Some(source_transition) = source_machine.transitions.iter().find(|transition| {
                transition.method_name == method_name
                    && source_machine
                        .state(transition.from)
                        .is_some_and(|state| state.rust_name == source_state_name)
            }) else {
                continue;
            };

            let transition_context = format!(
                "transition {}::{}",
                source_machine.rust_type_path, method_name
            );
            let relation_source = HeuristicRelationSource::Transition {
                machine: source_machine_index,
                transition: source_transition.index,
            };

            self.collect_method_signature_relations(
                file_path,
                source,
                module_path,
                imports,
                &transition_context,
                &relation_source,
                method,
                local_types,
            );

            let body_evidence = collect_body_evidence(method);
            for evidence in body_evidence {
                self.try_record_evidence(
                    file_path,
                    source,
                    module_path,
                    imports,
                    &transition_context,
                    &relation_source,
                    HeuristicEvidenceKind::Body,
                    evidence,
                );
            }
        }
    }

    fn scan_method_impl(
        &mut self,
        file_path: &Path,
        source: &str,
        module_path: &str,
        imports: &UseMap,
        local_types: &LocalTypeRegistry<'_>,
        item_impl: &ItemImpl,
    ) {
        if has_cfg_attrs(&item_impl.attrs) {
            return;
        }

        let Some((source_machine_index, source_state_index, _source_state_name)) = self
            .inventory
            .resolve_state_impl(module_path, &item_impl.self_ty)
        else {
            return;
        };
        let source_machine = self
            .doc
            .machine(source_machine_index)
            .expect("resolved heuristic source machine should exist");

        for item in &item_impl.items {
            let ImplItem::Fn(method) = item else {
                continue;
            };
            if has_cfg_attrs(&method.attrs) {
                continue;
            }

            let method_name = method.sig.ident.to_string();
            let method_context =
                format!("method {}::{}", source_machine.rust_type_path, method_name);
            let relation_source = HeuristicRelationSource::Method {
                machine: source_machine_index,
                state: source_state_index,
                method_name,
            };
            self.collect_method_signature_relations(
                file_path,
                source,
                module_path,
                imports,
                &method_context,
                &relation_source,
                method,
                local_types,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_method_signature_relations(
        &mut self,
        file_path: &Path,
        source: &str,
        module_path: &str,
        imports: &UseMap,
        context: &str,
        relation_source: &HeuristicRelationSource,
        method: &ImplItemFn,
        local_types: &LocalTypeRegistry<'_>,
    ) {
        for evidence in collect_method_type_surface_evidence(method, local_types) {
            self.try_record_evidence(
                file_path,
                source,
                module_path,
                imports,
                context,
                relation_source,
                HeuristicEvidenceKind::Signature,
                evidence,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_type_surface_relations(
        &mut self,
        file_path: &Path,
        source: &str,
        module_path: &str,
        imports: &UseMap,
        context: &str,
        relation_source: &HeuristicRelationSource,
        ty: &Type,
        local_types: &LocalTypeRegistry<'_>,
    ) {
        for evidence in collect_type_surface_evidence(ty, local_types) {
            self.try_record_evidence(
                file_path,
                source,
                module_path,
                imports,
                context,
                relation_source,
                HeuristicEvidenceKind::Signature,
                evidence,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn try_record_evidence(
        &mut self,
        file_path: &Path,
        source: &str,
        module_path: &str,
        imports: &UseMap,
        transition_context: &str,
        relation_source: &HeuristicRelationSource,
        evidence_kind: HeuristicEvidenceKind,
        evidence: PathEvidence,
    ) {
        let Some(resolved_path) = resolve_path_text(&evidence.path_text, module_path, imports)
        else {
            return;
        };

        match self.inventory.resolve_target_machine(&resolved_path) {
            ResolveTargetMachine::Unique(target_machine_index) => {
                if target_machine_index == relation_source.machine() {
                    return;
                }

                let key = HeuristicRelationKey {
                    source: relation_source.clone(),
                    target_machine: target_machine_index,
                    evidence_kind,
                    file_path: file_path.to_path_buf(),
                    line_number: evidence.line_number,
                };
                let candidate = HeuristicRelationCandidate {
                    matched_path_text: evidence.path_text,
                    snippet: source_line_snippet(source, evidence.line_number),
                };
                match self.relations.get_mut(&key) {
                    Some(existing)
                        if candidate.matched_path_text.len() > existing.matched_path_text.len() =>
                    {
                        *existing = candidate;
                    }
                    None => {
                        self.relations.insert(key, candidate);
                    }
                    Some(_) => {}
                }
            }
            ResolveTargetMachine::Ambiguous {
                candidate,
                machine_indices,
            } => {
                let machine_labels = machine_indices
                    .into_iter()
                    .filter_map(|index| self.doc.machine(index))
                    .map(|machine| machine.rust_type_path.to_owned())
                    .collect::<Vec<_>>()
                    .join(", ");
                self.push_diagnostic(
                    transition_context.to_owned(),
                    format!(
                        "ambiguous heuristic target for `{}` via `{candidate}`; matches {machine_labels}",
                        evidence.path_text
                    ),
                );
            }
            ResolveTargetMachine::NoCandidate => {}
        }
    }

    fn push_diagnostic(&mut self, context: impl Into<String>, message: impl Into<String>) {
        self.diagnostics.push(HeuristicDiagnostic {
            context: context.into(),
            message: message.into(),
        });
    }

    fn finish(self) -> HeuristicOverlay {
        let relations = self
            .relations
            .into_iter()
            .enumerate()
            .map(
                |(
                    index,
                    (
                        key,
                        HeuristicRelationCandidate {
                            matched_path_text,
                            snippet,
                        },
                    ),
                )| HeuristicRelation {
                    index,
                    source: key.source,
                    target_machine: key.target_machine,
                    evidence_kind: key.evidence_kind,
                    matched_path_text,
                    file_path: key.file_path,
                    line_number: key.line_number,
                    snippet,
                },
            )
            .collect::<Vec<_>>();

        let status = if self.scanned_files == 0 {
            HeuristicStatusKind::Unavailable
        } else if self.diagnostics.is_empty() {
            HeuristicStatusKind::Available
        } else {
            HeuristicStatusKind::Partial
        };

        HeuristicOverlay {
            status,
            diagnostics: self.diagnostics,
            relations,
        }
    }
}

struct MachineInventory<'a> {
    doc: &'a CodebaseDoc,
    by_module_path: HashMap<&'a str, Vec<usize>>,
}

impl<'a> MachineInventory<'a> {
    fn new(doc: &'a CodebaseDoc) -> Self {
        let mut by_module_path = HashMap::<&'a str, Vec<usize>>::new();
        for machine in doc.machines() {
            by_module_path
                .entry(machine.module_path)
                .or_default()
                .push(machine.index);
        }
        Self {
            doc,
            by_module_path,
        }
    }

    fn resolve_state_impl(
        &self,
        module_path: &str,
        self_ty: &Type,
    ) -> Option<(usize, usize, String)> {
        let (machine_name, state_name) = parse_machine_self_ty(self_ty)?;
        let module_path = strip_crate_prefix(module_path);
        let candidates = self.by_module_path.get(module_path)?;
        let machine = unique_machine(
            candidates
                .iter()
                .copied()
                .filter_map(|index| self.doc.machine(index))
                .filter(|machine| rust_type_leaf(machine.rust_type_path) == machine_name),
        )?;
        let state = machine.state_named(&state_name)?;
        Some((machine.index, state.index, state_name))
    }

    fn resolve_machine_in_module(&self, module_path: &str) -> Option<&'a CodebaseMachine> {
        let module_path = strip_crate_prefix(module_path);
        let candidates = self.by_module_path.get(module_path)?;
        unique_machine(
            candidates
                .iter()
                .copied()
                .filter_map(|index| self.doc.machine(index)),
        )
    }

    fn resolve_target_machine(&self, resolved_path: &str) -> ResolveTargetMachine {
        for candidate in candidate_module_prefixes(resolved_path) {
            let machine_indices = self.match_module_candidate(&candidate);
            match machine_indices.len() {
                0 => {}
                1 => return ResolveTargetMachine::Unique(machine_indices[0]),
                _ => {
                    return ResolveTargetMachine::Ambiguous {
                        candidate,
                        machine_indices,
                    };
                }
            }
        }

        ResolveTargetMachine::NoCandidate
    }

    fn match_module_candidate(&self, candidate: &str) -> Vec<usize> {
        self.doc
            .machines()
            .iter()
            .filter(|machine| {
                machine.module_path == candidate
                    || machine
                        .module_path
                        .strip_prefix(candidate)
                        .is_some_and(|rest| rest.starts_with("::"))
            })
            .map(|machine| machine.index)
            .collect()
    }
}

enum ResolveTargetMachine {
    Unique(usize),
    Ambiguous {
        candidate: String,
        machine_indices: Vec<usize>,
    },
    NoCandidate,
}

#[derive(Clone, Debug)]
struct PathEvidence {
    path_text: String,
    line_number: usize,
}

#[derive(Clone, Debug)]
struct HeuristicRelationCandidate {
    matched_path_text: String,
    snippet: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct HeuristicRelationKey {
    source: HeuristicRelationSource,
    target_machine: usize,
    evidence_kind: HeuristicEvidenceKind,
    file_path: PathBuf,
    line_number: usize,
}

fn build_use_map(items: &[Item], current_module: &str) -> UseMap {
    let mut imports = UseMap::new();
    for item in items {
        let Item::Use(item_use) = item else {
            continue;
        };
        if has_cfg_attrs(&item_use.attrs) {
            continue;
        }
        collect_use_tree(current_module, &item_use.tree, &mut imports, &[]);
    }
    imports
}

fn collect_use_tree(current_module: &str, tree: &UseTree, imports: &mut UseMap, prefix: &[String]) {
    match tree {
        UseTree::Path(use_path) => {
            let mut next_prefix = prefix.to_vec();
            next_prefix.push(use_path.ident.to_string());
            collect_use_tree(current_module, &use_path.tree, imports, &next_prefix);
        }
        UseTree::Name(use_name) => {
            if use_name.ident == "self" {
                let Some(alias) = prefix.last() else {
                    return;
                };
                if let Some(path) = resolve_use_path(current_module, prefix) {
                    imports.insert(alias.clone(), path);
                }
            } else {
                let mut full_path = prefix.to_vec();
                full_path.push(use_name.ident.to_string());
                if let Some(path) = resolve_use_path(current_module, &full_path) {
                    imports.insert(use_name.ident.to_string(), path);
                }
            }
        }
        UseTree::Rename(use_rename) => {
            let mut full_path = prefix.to_vec();
            full_path.push(use_rename.ident.to_string());
            if let Some(path) = resolve_use_path(current_module, &full_path) {
                imports.insert(use_rename.rename.to_string(), path);
            }
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_use_tree(current_module, item, imports, prefix);
            }
        }
        UseTree::Glob(_) => {}
    }
}

fn resolve_use_path(current_module: &str, segments: &[String]) -> Option<String> {
    resolve_path_segments(current_module, segments)
}

fn resolve_path_text(path_text: &str, current_module: &str, imports: &UseMap) -> Option<String> {
    let parsed = syn::parse_str::<syn::Path>(path_text).ok()?;
    resolve_syn_path(&parsed, current_module, imports)
}

fn resolve_syn_path(path: &syn::Path, current_module: &str, imports: &UseMap) -> Option<String> {
    let mut segments = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return None;
    }
    if path.leading_colon.is_some() {
        return Some(segments.join("::"));
    }

    if let Some(imported) = imports.get(&segments[0]) {
        let mut resolved = imported.split("::").map(str::to_owned).collect::<Vec<_>>();
        resolved.extend(segments.drain(1..));
        return Some(resolved.join("::"));
    }

    resolve_path_segments(current_module, &segments)
}

fn resolve_path_segments(current_module: &str, segments: &[String]) -> Option<String> {
    let first = segments.first()?;
    match first.as_str() {
        "crate" => Some(segments.join("::")),
        "self" => {
            let mut resolved = current_module
                .split("::")
                .map(str::to_owned)
                .collect::<Vec<_>>();
            resolved.extend(segments.iter().skip(1).cloned());
            Some(resolved.join("::"))
        }
        "super" => {
            let mut resolved = current_module
                .split("::")
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let super_count = segments
                .iter()
                .take_while(|segment| segment.as_str() == "super")
                .count();
            for _ in 0..super_count {
                if resolved.len() <= 1 {
                    return None;
                }
                resolved.pop();
            }
            resolved.extend(segments.iter().skip(super_count).cloned());
            Some(resolved.join("::"))
        }
        _ => Some(segments.join("::")),
    }
}

fn parse_machine_self_ty(self_ty: &Type) -> Option<(String, String)> {
    let Type::Path(type_path) = self_ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    let state = arguments.args.iter().find_map(|argument| {
        let syn::GenericArgument::Type(Type::Path(state_path)) = argument else {
            return None;
        };
        state_path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
    })?;
    Some((segment.ident.to_string(), state))
}

fn rust_type_leaf(rust_type_path: &str) -> &str {
    rust_type_path.rsplit("::").next().unwrap_or(rust_type_path)
}

fn unique_machine<'a>(
    mut candidates: impl Iterator<Item = &'a CodebaseMachine>,
) -> Option<&'a CodebaseMachine> {
    let first = candidates.next()?;
    if candidates.next().is_some() {
        None
    } else {
        Some(first)
    }
}

fn candidate_module_prefixes(resolved_path: &str) -> Vec<String> {
    let normalized = resolved_path.trim_start_matches("::");
    let primary = normalized
        .split("::")
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if primary.is_empty() {
        return Vec::new();
    }

    let mut candidates = BTreeSet::new();
    push_prefix_candidates(&primary, &mut candidates);
    if primary.len() > 1 {
        push_prefix_candidates(&primary[1..], &mut candidates);
    }

    candidates.into_iter().rev().collect()
}

fn push_prefix_candidates(segments: &[String], candidates: &mut BTreeSet<String>) {
    for length in (1..=segments.len()).rev() {
        let candidate = segments[..length].join("::");
        if !candidate.is_empty() {
            candidates.insert(candidate);
        }
    }
}

fn collect_method_type_surface_evidence(
    method: &ImplItemFn,
    local_types: &LocalTypeRegistry<'_>,
) -> Vec<PathEvidence> {
    let mut visitor = TypeSurfaceVisitor::new(local_types);
    for input in &method.sig.inputs {
        if let FnArg::Typed(input) = input {
            visitor.visit_pat_type(input);
        }
    }
    if let ReturnType::Type(_, ty) = &method.sig.output {
        visitor.visit_type(ty);
    }
    visitor.items
}

fn collect_type_surface_evidence(
    ty: &Type,
    local_types: &LocalTypeRegistry<'_>,
) -> Vec<PathEvidence> {
    let mut visitor = TypeSurfaceVisitor::new(local_types);
    visitor.visit_type(ty);
    visitor.items
}

fn collect_body_evidence(method: &ImplItemFn) -> Vec<PathEvidence> {
    let mut visitor = BodyPathVisitor { items: Vec::new() };
    visitor.visit_block(&method.block);
    visitor.items
}

struct TypeSurfaceVisitor<'a> {
    items: Vec<PathEvidence>,
    local_types: &'a LocalTypeRegistry<'a>,
    visited_local_types: HashSet<String>,
}

impl<'a> TypeSurfaceVisitor<'a> {
    fn new(local_types: &'a LocalTypeRegistry<'a>) -> Self {
        Self {
            items: Vec::new(),
            local_types,
            visited_local_types: HashSet::new(),
        }
    }
}

impl<'ast> Visit<'ast> for TypeSurfaceVisitor<'_> {
    fn visit_type_path(&mut self, node: &'ast TypePath) {
        if node.qself.is_none() {
            self.items.push(PathEvidence {
                path_text: node.path.to_token_stream().to_string(),
                line_number: node.path.span().start().line,
            });

            if let Some(item_struct) = self.local_types.single_segment_struct(node) {
                let struct_name = item_struct.ident.to_string();
                if self.visited_local_types.insert(struct_name) {
                    for field in &item_struct.fields {
                        if has_cfg_attrs(&field.attrs) {
                            continue;
                        }
                        self.visit_type(&field.ty);
                    }
                }
            }
        }
        visit::visit_type_path(self, node);
    }
}

struct BodyPathVisitor {
    items: Vec<PathEvidence>,
}

impl<'ast> Visit<'ast> for BodyPathVisitor {
    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if node.qself.is_none()
            && (node.path.leading_colon.is_some() || node.path.segments.len() > 1)
        {
            self.items.push(PathEvidence {
                path_text: node.path.to_token_stream().to_string(),
                line_number: node.path.span().start().line,
            });
        }
        visit::visit_expr_path(self, node);
    }

    fn visit_type_path(&mut self, node: &'ast TypePath) {
        if node.qself.is_none() {
            self.items.push(PathEvidence {
                path_text: node.path.to_token_stream().to_string(),
                line_number: node.path.span().start().line,
            });
        }
        visit::visit_type_path(self, node);
    }
}

fn source_line_snippet(source: &str, line_number: usize) -> Option<String> {
    if line_number == 0 {
        return None;
    }
    source
        .lines()
        .nth(line_number.saturating_sub(1))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
}

fn has_state_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attribute| {
        matches!(attribute.style, AttrStyle::Outer) && attribute.path().is_ident("state")
    })
}

fn has_transition_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attribute| {
        matches!(attribute.style, AttrStyle::Outer) && attribute.path().is_ident("transition")
    })
}

fn has_cfg_attrs(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attribute| {
        matches!(attribute.style, AttrStyle::Outer)
            && (attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr"))
    })
}

fn explicit_module_file_path(item_mod: &ItemMod, file_path: &Path) -> Option<PathBuf> {
    let attr = item_mod.attrs.iter().find(|attribute| {
        matches!(attribute.style, AttrStyle::Outer) && attribute.path().is_ident("path")
    })?;
    let meta = attr.meta.require_name_value().ok()?;
    let syn::Expr::Lit(expr_lit) = &meta.value else {
        return None;
    };
    let syn::Lit::Str(path) = &expr_lit.lit else {
        return None;
    };
    Some(
        file_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path.value()),
    )
}

fn join_module_path(parent: &str, child: &str) -> String {
    if parent == "crate" {
        format!("crate::{child}")
    } else {
        format!("{parent}::{child}")
    }
}

fn strip_crate_prefix(path: &str) -> &str {
    path.strip_prefix("crate::").unwrap_or(path)
}

fn normalize_absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .expect("current directory should exist")
            .join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    use statum::{machine, state, transition};
    use tempfile::tempdir;

    #[allow(dead_code)]
    mod heuristics_fixture {
        pub mod heuristic_task {
            use super::super::{machine, state, transition};

            #[state]
            pub enum State {
                Idle,
                Running,
                Done,
            }

            #[machine]
            pub struct Machine<State> {}

            #[transition]
            impl Machine<Idle> {
                fn start(self) -> Machine<Running> {
                    self.transition()
                }
            }

            #[transition]
            impl Machine<Running> {
                fn finish(self) -> Machine<Done> {
                    self.transition()
                }
            }
        }

        pub mod heuristic_workflow {
            use super::super::{machine, state, transition};
            use super::heuristic_task;

            #[state]
            pub enum State {
                Draft,
                InProgress,
                Done,
            }

            #[machine]
            pub struct Machine<State> {}

            #[transition]
            impl Machine<Draft> {
                fn start(
                    self,
                    task: heuristic_task::Machine<heuristic_task::Running>,
                ) -> Machine<InProgress> {
                    let _ = task;
                    self.transition()
                }
            }

            #[transition]
            impl Machine<InProgress> {
                fn finish(self) -> Machine<Done> {
                    self.transition()
                }
            }
        }
    }

    #[allow(dead_code)]
    mod ambiguous_fixture {
        pub mod workflow {
            use super::super::{machine, state, transition};

            #[state]
            pub enum State {
                Draft,
                Done,
            }

            #[machine]
            pub struct Machine<State> {}

            #[transition]
            impl Machine<Draft> {
                fn start(self) -> Machine<Done> {
                    self.transition()
                }
            }
        }

        pub mod flows {
            pub mod task {
                pub mod alpha {
                    use super::super::super::super::{machine, state};

                    #[state]
                    pub enum State {
                        Ready,
                    }

                    #[machine]
                    pub struct Machine<State> {}
                }

                pub mod beta {
                    use super::super::super::super::{machine, state};

                    #[state]
                    pub enum State {
                        Ready,
                    }

                    #[machine]
                    pub struct Machine<State> {}
                }
            }
        }
    }

    fn fixture_doc() -> CodebaseDoc {
        CodebaseDoc::linked().expect("linked codebase doc")
    }

    fn write_package_sources(
        dir: &Path,
        lib_rs: &str,
        extra_files: &[(&str, &str)],
    ) -> InspectPackageSource {
        fs::create_dir_all(dir.join("src")).expect("fixture src dir");
        fs::write(dir.join("src/lib.rs"), lib_rs).expect("fixture lib.rs");
        for (relative, contents) in extra_files {
            let path = dir.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("fixture parent dir");
            }
            fs::write(path, contents).expect("fixture source file");
        }

        InspectPackageSource {
            package_name: "fixture-app".to_owned(),
            manifest_dir: dir.to_path_buf(),
            lib_target_path: dir.join("src/lib.rs"),
        }
    }

    #[test]
    fn collects_signature_and_body_relations_from_transition_sources() {
        let dir = tempdir().expect("fixture tempdir");
        let package = write_package_sources(
            dir.path(),
            "pub mod heuristics {\n    pub mod tests {\n        pub mod heuristics_fixture;\n    }\n}\n",
            &[
                (
                    "src/heuristics/tests/heuristics_fixture/mod.rs",
                    "pub mod heuristic_task;\npub mod heuristic_workflow;\n",
                ),
                (
                    "src/heuristics/tests/heuristics_fixture/heuristic_task.rs",
                    "pub struct Receipt;\n\
                     pub struct Idle;\n\
                     pub struct Running;\n\
                     pub struct Done;\n\
                     pub struct Machine<State>(std::marker::PhantomData<State>);\n",
                ),
                (
                    "src/heuristics/tests/heuristics_fixture/heuristic_workflow.rs",
                    "use super::heuristic_task;\n\
                     use statum::transition;\n\
                     pub struct Draft;\n\
                     pub struct InProgress;\n\
                     pub struct Done;\n\
                     pub struct Machine<State>(std::marker::PhantomData<State>);\n\
                     #[transition]\n\
                     impl Machine<Draft> {\n\
                         fn start(self, task: heuristic_task::Machine<heuristic_task::Running>) -> Machine<InProgress> {\n\
                             let _receipt = heuristic_task::Receipt;\n\
                             let _builder = heuristic_task::Machine::<heuristic_task::Running>;\n\
                             self\n\
                         }\n\
                     }\n",
                ),
            ],
        );

        let overlay = collect_heuristic_overlay(&fixture_doc(), &[package]);

        assert_eq!(overlay.status(), HeuristicStatusKind::Available);
        assert_eq!(overlay.relations().len(), 3);
        assert_eq!(
            overlay
                .relations()
                .iter()
                .map(|relation| relation.evidence_kind)
                .collect::<Vec<_>>(),
            vec![
                HeuristicEvidenceKind::Signature,
                HeuristicEvidenceKind::Body,
                HeuristicEvidenceKind::Body
            ]
        );
    }

    #[test]
    fn ambiguous_module_affinity_fails_closed_with_diagnostic() {
        let dir = tempdir().expect("fixture tempdir");
        let package = write_package_sources(
            dir.path(),
            "pub mod heuristics {\n    pub mod tests {\n        pub mod ambiguous_fixture;\n    }\n}\n",
            &[
                (
                    "src/heuristics/tests/ambiguous_fixture/mod.rs",
                    "pub mod workflow;\npub mod flows;\n",
                ),
                (
                    "src/heuristics/tests/ambiguous_fixture/workflow.rs",
                    "use statum::transition;\n\
                     pub struct Draft;\n\
                     pub struct Done;\n\
                     pub struct Machine<State>(std::marker::PhantomData<State>);\n\
                     #[transition]\n\
                     impl Machine<Draft> {\n\
                         fn start(self) -> Machine<Done> {\n\
                             let _receipt = crate::heuristics::tests::ambiguous_fixture::flows::task::Receipt;\n\
                             self\n\
                         }\n\
                     }\n",
                ),
                (
                    "src/heuristics/tests/ambiguous_fixture/flows/mod.rs",
                    "pub mod task;\n",
                ),
                (
                    "src/heuristics/tests/ambiguous_fixture/flows/task/mod.rs",
                    "pub mod alpha;\npub mod beta;\npub struct Receipt;\n",
                ),
                (
                    "src/heuristics/tests/ambiguous_fixture/flows/task/alpha.rs",
                    "pub struct Ready;\npub struct Machine<State>(std::marker::PhantomData<State>);\n",
                ),
                (
                    "src/heuristics/tests/ambiguous_fixture/flows/task/beta.rs",
                    "pub struct Ready;\npub struct Machine<State>(std::marker::PhantomData<State>);\n",
                ),
            ],
        );

        let overlay = collect_heuristic_overlay(&fixture_doc(), &[package]);

        assert_eq!(overlay.status(), HeuristicStatusKind::Partial);
        assert!(overlay.relations().is_empty());
        assert!(overlay
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("ambiguous heuristic target")));
    }

    #[test]
    fn cfg_decorated_transition_methods_are_skipped() {
        let dir = tempdir().expect("fixture tempdir");
        let package = write_package_sources(
            dir.path(),
            "pub mod heuristics {\n    pub mod tests {\n        pub mod heuristics_fixture;\n    }\n}\n",
            &[
                (
                    "src/heuristics/tests/heuristics_fixture/mod.rs",
                    "pub mod heuristic_task;\npub mod heuristic_workflow;\n",
                ),
                (
                    "src/heuristics/tests/heuristics_fixture/heuristic_task.rs",
                    "pub struct Running;\n\
                     pub struct Machine<State>(std::marker::PhantomData<State>);\n",
                ),
                (
                    "src/heuristics/tests/heuristics_fixture/heuristic_workflow.rs",
                    "use super::heuristic_task;\n\
                     use statum::transition;\n\
                     pub struct Draft;\n\
                     pub struct Done;\n\
                     pub struct Machine<State>(std::marker::PhantomData<State>);\n\
                     #[transition]\n\
                     impl Machine<Draft> {\n\
                         #[cfg(any())]\n\
                         fn start(self, task: heuristic_task::Machine<heuristic_task::Running>) -> Machine<Done> {\n\
                             let _ = task;\n\
                             self\n\
                         }\n\
                     }\n",
                ),
            ],
        );

        let overlay = collect_heuristic_overlay(&fixture_doc(), &[package]);

        assert_eq!(overlay.status(), HeuristicStatusKind::Available);
        assert!(overlay.relations().is_empty());
    }

    #[test]
    fn body_variable_uses_do_not_count_as_explicit_body_paths() {
        let dir = tempdir().expect("fixture tempdir");
        let package = write_package_sources(
            dir.path(),
            "pub mod heuristics {\n    pub mod tests {\n        pub mod heuristics_fixture;\n    }\n}\n",
            &[
                (
                    "src/heuristics/tests/heuristics_fixture/mod.rs",
                    "pub mod heuristic_task;\npub mod heuristic_workflow;\n",
                ),
                (
                    "src/heuristics/tests/heuristics_fixture/heuristic_task.rs",
                    "pub struct Running;\n\
                     pub struct Machine<State>(std::marker::PhantomData<State>);\n",
                ),
                (
                    "src/heuristics/tests/heuristics_fixture/heuristic_workflow.rs",
                    "use super::heuristic_task;\n\
                     use statum::transition;\n\
                     pub struct Draft;\n\
                     pub struct Done;\n\
                     pub struct Machine<State>(std::marker::PhantomData<State>);\n\
                     #[transition]\n\
                     impl Machine<Draft> {\n\
                         fn start(self, task: heuristic_task::Machine<heuristic_task::Running>) -> Machine<Done> {\n\
                             let _ = task;\n\
                             self\n\
                         }\n\
                     }\n",
                ),
            ],
        );

        let overlay = collect_heuristic_overlay(&fixture_doc(), &[package]);

        assert_eq!(overlay.status(), HeuristicStatusKind::Available);
        assert_eq!(overlay.relations().len(), 1);
        assert_eq!(
            overlay.relations()[0].evidence_kind,
            HeuristicEvidenceKind::Signature
        );
    }

    #[test]
    fn path_attribute_modules_are_scanned_without_unavailable_diagnostics() {
        let dir = tempdir().expect("fixture tempdir");
        let package = write_package_sources(
            dir.path(),
            "#[path = \"support/fault.rs\"]\n\
             pub mod fault;\n\
             pub mod heuristics {\n    pub mod tests {\n        pub mod heuristics_fixture;\n    }\n}\n",
            &[
                ("src/support/fault.rs", "pub struct Error;\n"),
                (
                    "src/heuristics/tests/heuristics_fixture/mod.rs",
                    "pub mod heuristic_task;\npub mod heuristic_workflow;\n",
                ),
                (
                    "src/heuristics/tests/heuristics_fixture/heuristic_task.rs",
                    "pub struct Running;\n\
                     pub struct Machine<State>(std::marker::PhantomData<State>);\n",
                ),
                (
                    "src/heuristics/tests/heuristics_fixture/heuristic_workflow.rs",
                    "use super::heuristic_task;\n\
                     pub struct Draft;\n\
                     pub struct Machine<State>(std::marker::PhantomData<State>);\n\
                     impl Machine<Draft> {\n\
                         fn await_task(self, _task: heuristic_task::Machine<heuristic_task::Running>) -> Self {\n\
                             self\n\
                         }\n\
                     }\n",
                ),
            ],
        );

        let overlay = collect_heuristic_overlay(&fixture_doc(), &[package]);

        assert_eq!(overlay.status(), HeuristicStatusKind::Available);
        assert_eq!(overlay.diagnostics(), &[]);
    }

    #[test]
    fn collects_relations_from_non_transition_method_signatures() {
        let dir = tempdir().expect("fixture tempdir");
        let package = write_package_sources(
            dir.path(),
            "pub mod heuristics {\n    pub mod tests {\n        pub mod heuristics_fixture;\n    }\n}\n",
            &[
                (
                    "src/heuristics/tests/heuristics_fixture/mod.rs",
                    "pub mod heuristic_task;\npub mod heuristic_workflow;\n",
                ),
                (
                    "src/heuristics/tests/heuristics_fixture/heuristic_task.rs",
                    "pub struct Running;\n\
                     pub struct Machine<State>(std::marker::PhantomData<State>);\n",
                ),
                (
                    "src/heuristics/tests/heuristics_fixture/heuristic_workflow.rs",
                    "use super::heuristic_task;\n\
                     pub struct Draft;\n\
                     pub struct Done;\n\
                     pub struct Machine<State>(std::marker::PhantomData<State>);\n\
                     impl Machine<Draft> {\n\
                         fn await_task(self, _task: heuristic_task::Machine<heuristic_task::Running>) -> Machine<Done> {\n\
                             self\n\
                         }\n\
                     }\n",
                ),
            ],
        );

        let overlay = collect_heuristic_overlay(&fixture_doc(), &[package]);

        assert_eq!(overlay.status(), HeuristicStatusKind::Available);
        assert_eq!(overlay.relations().len(), 1);
        assert_eq!(
            overlay.relations()[0].evidence_kind,
            HeuristicEvidenceKind::Signature
        );
        assert!(matches!(
            overlay.relations()[0].source,
            HeuristicRelationSource::Method {
                state: 0,
                ref method_name,
                ..
            } if method_name == "await_task"
        ));
    }

    #[test]
    fn collects_relations_from_state_payload_struct_recursion() {
        let dir = tempdir().expect("fixture tempdir");
        let package = write_package_sources(
            dir.path(),
            "pub mod heuristics {\n    pub mod tests {\n        pub mod heuristics_fixture;\n    }\n}\n",
            &[
                (
                    "src/heuristics/tests/heuristics_fixture/mod.rs",
                    "pub mod heuristic_task;\npub mod heuristic_workflow;\n",
                ),
                (
                    "src/heuristics/tests/heuristics_fixture/heuristic_task.rs",
                    "pub struct Running;\n\
                     pub struct Machine<State>(std::marker::PhantomData<State>);\n",
                ),
                (
                    "src/heuristics/tests/heuristics_fixture/heuristic_workflow.rs",
                    "use super::heuristic_task;\n\
                     use statum::{machine, state};\n\
                     pub struct ReadyData {\n\
                         handoff: heuristic_task::Machine<heuristic_task::Running>,\n\
                     }\n\
                     #[state]\n\
                     pub enum State {\n\
                         Draft,\n\
                         InProgress(ReadyData),\n\
                         Done,\n\
                     }\n\
                     #[machine]\n\
                     pub struct Machine<State> {}\n",
                ),
            ],
        );

        let overlay = collect_heuristic_overlay(&fixture_doc(), &[package]);

        assert_eq!(overlay.status(), HeuristicStatusKind::Available);
        assert_eq!(overlay.relations().len(), 1);
        assert!(matches!(
            overlay.relations()[0].source,
            HeuristicRelationSource::State { state: 1, .. }
        ));
    }
}
