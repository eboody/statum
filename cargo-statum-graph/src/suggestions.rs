use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use statum_graph::{
    CodebaseDoc, CodebaseMachine, CodebaseRelation, CodebaseRelationBasis, CodebaseRelationCount,
};

use crate::heuristics::{HeuristicOverlay, HeuristicRelationCount};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositionSuggestionSeverity {
    Warning,
    Suggestion,
}

impl CompositionSuggestionSeverity {
    pub const fn display_label(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Suggestion => "suggestion",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositionSuggestionKind {
    MissingCompositionRole,
    HeuristicCompositionCandidate,
}

impl CompositionSuggestionKind {
    pub const fn display_label(self) -> &'static str {
        match self {
            Self::MissingCompositionRole => "missing composition role",
            Self::HeuristicCompositionCandidate => "heuristic composition candidate",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionSuggestion {
    pub index: usize,
    pub severity: CompositionSuggestionSeverity,
    pub kind: CompositionSuggestionKind,
    pub source_machine: usize,
    pub target_machine: usize,
    pub exact_relation_indices: Vec<usize>,
    pub heuristic_relation_indices: Vec<usize>,
    pub exact_counts: Vec<CodebaseRelationCount>,
    pub heuristic_counts: Vec<HeuristicRelationCount>,
}

impl CompositionSuggestion {
    pub fn source_machine<'a>(&self, doc: &'a CodebaseDoc) -> Option<&'a CodebaseMachine> {
        doc.machine(self.source_machine)
    }

    pub fn target_machine<'a>(&self, doc: &'a CodebaseDoc) -> Option<&'a CodebaseMachine> {
        doc.machine(self.target_machine)
    }

    pub fn summary_label(&self, doc: &CodebaseDoc) -> String {
        let source = self
            .source_machine(doc)
            .map(render_machine_label)
            .unwrap_or_else(|| "<missing machine>".to_owned());
        let target = self
            .target_machine(doc)
            .map(render_machine_label)
            .unwrap_or_else(|| "<missing machine>".to_owned());
        format!("{source} -> {target}")
    }

    pub fn counts_label(&self) -> String {
        match self.severity {
            CompositionSuggestionSeverity::Warning => self
                .exact_counts
                .iter()
                .map(CodebaseRelationCount::display_label)
                .collect::<Vec<_>>()
                .join(", "),
            CompositionSuggestionSeverity::Suggestion => self
                .heuristic_counts
                .iter()
                .map(HeuristicRelationCount::display_label)
                .collect::<Vec<_>>()
                .join(", "),
        }
    }

    pub const fn help_text(&self) -> &'static str {
        match self.kind {
            CompositionSuggestionKind::MissingCompositionRole => {
                "consider `#[machine(role = composition)]` on the source machine"
            }
            CompositionSuggestionKind::HeuristicCompositionCandidate => {
                "if this coupling is real workflow orchestration, model it in typed composition state/transition surfaces or promote a detached handoff into the exact lane"
            }
        }
    }

    pub const fn why_text(&self) -> &'static str {
        match self.kind {
            CompositionSuggestionKind::MissingCompositionRole => {
                "protocol machine already exposes typed cross-machine orchestration"
            }
            CompositionSuggestionKind::HeuristicCompositionCandidate => {
                "cross-machine coupling is still only visible through the heuristic lane"
            }
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompositionSuggestionOverlay {
    suggestions: Vec<CompositionSuggestion>,
}

impl CompositionSuggestionOverlay {
    pub fn suggestions(&self) -> &[CompositionSuggestion] {
        &self.suggestions
    }

    pub fn is_empty(&self) -> bool {
        self.suggestions.is_empty()
    }

    pub fn machine_suggestions(
        &self,
        machine_index: usize,
    ) -> impl Iterator<Item = &CompositionSuggestion> + '_ {
        self.suggestions
            .iter()
            .filter(move |suggestion| suggestion.source_machine == machine_index)
    }

    pub fn warning_count(&self) -> usize {
        self.suggestions
            .iter()
            .filter(|suggestion| suggestion.severity == CompositionSuggestionSeverity::Warning)
            .count()
    }

    pub fn suggestion_count(&self) -> usize {
        self.suggestions
            .iter()
            .filter(|suggestion| suggestion.severity == CompositionSuggestionSeverity::Suggestion)
            .count()
    }

    #[cfg(test)]
    pub(crate) fn from_suggestions(suggestions: Vec<CompositionSuggestion>) -> Self {
        Self { suggestions }
    }
}

pub fn collect_composition_suggestions(
    doc: &CodebaseDoc,
    heuristic: &HeuristicOverlay,
) -> CompositionSuggestionOverlay {
    let mut suggestions = Vec::new();
    let mut exact_pairs = BTreeSet::new();

    for group in doc.machine_relation_groups() {
        if group.from_machine == group.to_machine {
            continue;
        }
        let Some(source_machine) = doc.machine(group.from_machine) else {
            continue;
        };
        if source_machine.role.is_composition() {
            continue;
        }

        let mut candidate_relations = Vec::new();
        let mut counts =
            BTreeMap::<(statum_graph::CodebaseRelationKind, CodebaseRelationBasis), usize>::new();
        for relation_index in &group.relation_indices {
            let Some(relation) = doc.relation(*relation_index) else {
                continue;
            };
            if !is_high_confidence_typed_orchestration(relation) {
                continue;
            }
            candidate_relations.push(*relation_index);
            *counts.entry((relation.kind, relation.basis)).or_default() += 1;
        }

        if candidate_relations.is_empty() {
            continue;
        }

        exact_pairs.insert((group.from_machine, group.to_machine));
        suggestions.push(CompositionSuggestion {
            index: suggestions.len(),
            severity: CompositionSuggestionSeverity::Warning,
            kind: CompositionSuggestionKind::MissingCompositionRole,
            source_machine: group.from_machine,
            target_machine: group.to_machine,
            exact_relation_indices: candidate_relations,
            heuristic_relation_indices: Vec::new(),
            exact_counts: counts
                .into_iter()
                .map(|((kind, basis), count)| CodebaseRelationCount { kind, basis, count })
                .collect(),
            heuristic_counts: Vec::new(),
        });
    }

    let mut legacy_link_counts = BTreeMap::<(usize, usize), usize>::new();
    for link in doc.links() {
        *legacy_link_counts
            .entry((link.from_machine, link.to_machine))
            .or_default() += 1;
    }

    for ((from_machine, to_machine), count) in legacy_link_counts {
        if from_machine == to_machine || exact_pairs.contains(&(from_machine, to_machine)) {
            continue;
        }
        let Some(source_machine) = doc.machine(from_machine) else {
            continue;
        };
        if source_machine.role.is_composition() {
            continue;
        }

        exact_pairs.insert((from_machine, to_machine));
        suggestions.push(CompositionSuggestion {
            index: suggestions.len(),
            severity: CompositionSuggestionSeverity::Warning,
            kind: CompositionSuggestionKind::MissingCompositionRole,
            source_machine: from_machine,
            target_machine: to_machine,
            exact_relation_indices: Vec::new(),
            heuristic_relation_indices: Vec::new(),
            exact_counts: vec![CodebaseRelationCount {
                kind: statum_graph::CodebaseRelationKind::StatePayload,
                basis: CodebaseRelationBasis::DirectTypeSyntax,
                count,
            }],
            heuristic_counts: Vec::new(),
        });
    }

    for group in heuristic.machine_relation_groups() {
        if group.from_machine == group.to_machine {
            continue;
        }
        if exact_pairs.contains(&(group.from_machine, group.to_machine)) {
            continue;
        }
        let Some(source_machine) = doc.machine(group.from_machine) else {
            continue;
        };
        if source_machine.role.is_composition() {
            continue;
        }
        if doc.machine_relation_groups().iter().any(|exact| {
            exact.from_machine == group.from_machine && exact.to_machine == group.to_machine
        }) {
            continue;
        }

        suggestions.push(CompositionSuggestion {
            index: suggestions.len(),
            severity: CompositionSuggestionSeverity::Suggestion,
            kind: CompositionSuggestionKind::HeuristicCompositionCandidate,
            source_machine: group.from_machine,
            target_machine: group.to_machine,
            exact_relation_indices: Vec::new(),
            heuristic_relation_indices: group.relation_indices.clone(),
            exact_counts: Vec::new(),
            heuristic_counts: group.counts.clone(),
        });
    }

    CompositionSuggestionOverlay { suggestions }
}

pub fn render_composition_suggestions(doc: &CodebaseDoc, heuristic: &HeuristicOverlay) -> String {
    let overlay = collect_composition_suggestions(doc, heuristic);
    let mut output = String::new();
    let _ = writeln!(
        output,
        "composition diagnostics: {} warning, {} suggestion",
        overlay.warning_count(),
        overlay.suggestion_count()
    );
    let _ = writeln!(
        output,
        "heuristics: {} ({})",
        heuristic.status().display_label(),
        heuristic.diagnostics().len()
    );
    for diagnostic in heuristic.diagnostics().iter().take(3) {
        let _ = writeln!(
            output,
            "heuristic diagnostic: {}",
            diagnostic.display_label()
        );
    }

    if overlay.is_empty() {
        let _ = writeln!(output, "no composition diagnostics");
        return output;
    }

    for suggestion in overlay.suggestions() {
        let _ = writeln!(output);
        let _ = writeln!(
            output,
            "{}: {}",
            suggestion.severity.display_label(),
            suggestion.summary_label(doc)
        );
        let _ = writeln!(output, "kind: {}", suggestion.kind.display_label());
        let _ = writeln!(output, "why: {}", suggestion.why_text());
        let _ = writeln!(output, "evidence: {}", suggestion.counts_label());
        let _ = writeln!(output, "help: {}", suggestion.help_text());
    }

    output
}

fn is_high_confidence_typed_orchestration(relation: &CodebaseRelation) -> bool {
    matches!(
        relation.basis,
        CodebaseRelationBasis::DirectTypeSyntax
            | CodebaseRelationBasis::AttestedTypeSyntax
            | CodebaseRelationBasis::ViaDeclaration
    )
}

fn render_machine_label(machine: &CodebaseMachine) -> String {
    machine.label.unwrap_or(machine.rust_type_path).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heuristics::{
        HeuristicEvidenceKind, HeuristicRelation, HeuristicRelationSource, HeuristicStatusKind,
    };

    mod suggestion_task {
        use statum::{machine, state};

        #[state]
        pub enum State {
            Running,
        }

        #[machine]
        pub struct Machine<State> {}
    }

    mod suggestion_workflow {
        use super::suggestion_task as task;
        use statum::{machine, state, transition};

        #[state]
        pub enum State {
            Draft,
            InProgress(task::Machine<task::Running>),
        }

        #[machine]
        pub struct Machine<State> {}

        #[allow(dead_code)]
        #[transition]
        impl Machine<Draft> {
            fn start(self, task: task::Machine<task::Running>) -> Machine<InProgress> {
                self.transition_with(task)
            }
        }
    }

    fn fixture_doc() -> CodebaseDoc {
        CodebaseDoc::linked().expect("linked doc")
    }

    #[test]
    fn exact_protocol_machine_with_typed_child_machine_gets_warning() {
        let doc = fixture_doc();
        let overlay = collect_composition_suggestions(
            &doc,
            &HeuristicOverlay::from_parts(HeuristicStatusKind::Available, Vec::new(), Vec::new()),
        );

        assert!(overlay.warning_count() >= 1);
        let suggestion = overlay
            .suggestions()
            .iter()
            .find(|suggestion| {
                suggestion.severity == CompositionSuggestionSeverity::Warning
                    && suggestion.kind == CompositionSuggestionKind::MissingCompositionRole
            })
            .expect("composition warning");
        assert_eq!(
            suggestion.kind,
            CompositionSuggestionKind::MissingCompositionRole
        );
        assert!(!suggestion.exact_counts.is_empty());
    }

    #[test]
    fn heuristic_only_protocol_machine_gets_suggestion() {
        let doc = fixture_doc();
        let task = doc
            .machines()
            .iter()
            .find(|machine| machine.rust_type_path.ends_with("suggestion_task::Machine"))
            .expect("task");
        let workflow = doc
            .machines()
            .iter()
            .find(|machine| {
                machine
                    .rust_type_path
                    .ends_with("suggestion_workflow::Machine")
            })
            .expect("workflow");

        let overlay = collect_composition_suggestions(
            &doc,
            &HeuristicOverlay::from_parts(
                HeuristicStatusKind::Available,
                Vec::new(),
                vec![HeuristicRelation {
                    index: 0,
                    source: HeuristicRelationSource::Transition {
                        machine: task.index,
                        transition: 0,
                    },
                    target_machine: workflow.index,
                    evidence_kind: HeuristicEvidenceKind::Signature,
                    matched_path_text:
                        "suggestion_workflow::Machine<suggestion_workflow::InProgress>".to_owned(),
                    file_path: "/tmp/task.rs".into(),
                    line_number: 10,
                    snippet: None,
                }],
            ),
        );

        assert!(overlay.warning_count() >= 1);
        assert!(overlay.suggestion_count() >= 1);
        let suggestion = overlay
            .suggestions()
            .iter()
            .find(|suggestion| suggestion.severity == CompositionSuggestionSeverity::Suggestion)
            .expect("heuristic suggestion");
        assert_eq!(
            suggestion.kind,
            CompositionSuggestionKind::HeuristicCompositionCandidate
        );
    }

    #[test]
    fn rendered_report_includes_heuristic_status() {
        let doc = fixture_doc();
        let report = render_composition_suggestions(
            &doc,
            &HeuristicOverlay::from_parts(
                HeuristicStatusKind::Partial,
                vec![crate::heuristics::HeuristicDiagnostic {
                    context: "package fixture".to_owned(),
                    message: "failed to parse one module".to_owned(),
                }],
                Vec::new(),
            ),
        );

        assert!(report.contains("heuristics: partial (1)"));
        assert!(report.contains("heuristic diagnostic:"));
    }
}
