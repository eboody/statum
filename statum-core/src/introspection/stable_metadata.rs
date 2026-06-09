use super::MachineGraph;

/// Stable schema version for exported graph metadata.
///
/// The JSON representation is a lower-case string so external tools can branch
/// on it without depending on Rust enum names.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StableGraphMetadataVersion {
    /// Initial stable graph metadata shape.
    #[serde(rename = "v1")]
    V1,
}

/// Semantic authority level claimed by a `StableGraphMetadata` document.
///
/// This records the observation point for the generated graph. It is not a
/// claim over arbitrary Rust semantics or runtime behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphAuthorityLevel {
    /// The graph was observed from the cfg-pruned attribute macro input and the
    /// supported return-type shapes Statum knows how to interpret.
    CfgPrunedMacroInput,
}

/// Known semantic cases this stable graph metadata model does not claim to
/// represent.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnsupportedGraphMetadataCase {
    /// Transitions created or selected only by runtime values are outside the
    /// static graph. Runtime recordings can be joined back to known transition
    /// ids, but they cannot add new graph edges.
    RuntimeOnlyTransitions,
    /// Cfg-pruned source is the observation point. Ambiguous aliases or return
    /// shapes that depend on unsupported nested cfg patterns are rejected before
    /// stable metadata is produced.
    CfgAmbiguousAliases,
    /// Custom decision enums or aliases that are not one of Statum's supported
    /// wrapper shapes are rejected before this metadata is produced.
    UnexpandedCustomDecisionEnums,
    /// Items or aliases produced only by macro expansion are outside Statum's
    /// source-level transition target observation point.
    MacroGeneratedItems,
    /// Items or aliases produced only by `include!` expansion are outside
    /// Statum's source-level transition target observation point.
    IncludeGeneratedItems,
    /// Field-level labels/descriptions/metadata are reserved in the JSON shape,
    /// but current graph emission does not populate them.
    FieldLevelPresentationMetadata,
}

/// Stable Rust and JSON metadata shape for one Statum machine graph.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct StableGraphMetadata {
    /// Schema version for this document.
    pub version: StableGraphMetadataVersion,
    /// Semantic observation point the graph is allowed to claim.
    pub authority: GraphAuthorityLevel,
    /// Cases intentionally absent from this shape or rejected by macro
    /// generation before a graph is emitted.
    pub unsupported_cases: Vec<UnsupportedGraphMetadataCase>,
    /// Machine-level metadata.
    pub machine: StableMachineMetadata,
    /// State metadata in graph order.
    pub states: Vec<StableStateMetadata>,
    /// Transition metadata in graph order.
    pub transitions: Vec<StableTransitionMetadata>,
}

/// Stable machine-readable graph invariant lint code.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphLintCode {
    /// A terminal-looking state name owns an outgoing transition.
    TerminalStateHasOutgoingTransition,
    /// A state cannot be reached from the first exported state through exported transitions.
    UnreachableState,
    /// A non-initial state has no incoming exported transition.
    MissingIncomingPath,
    /// A transition can return to its own source state.
    SuspiciousSelfTransition,
}

/// One graph invariant lint finding produced from stable metadata.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct GraphLintFinding {
    /// Stable machine-readable lint code.
    pub code: GraphLintCode,
    /// Human-readable lint explanation.
    pub message: String,
    /// State associated with the finding when applicable.
    pub state: Option<String>,
    /// Transition associated with the finding when applicable.
    pub transition: Option<String>,
}

impl GraphLintCode {
    fn as_str(self) -> &'static str {
        match self {
            Self::TerminalStateHasOutgoingTransition => "terminal_state_has_outgoing_transition",
            Self::UnreachableState => "unreachable_state",
            Self::MissingIncomingPath => "missing_incoming_path",
            Self::SuspiciousSelfTransition => "suspicious_self_transition",
        }
    }
}

impl GraphAuthorityLevel {
    fn as_str(self) -> &'static str {
        match self {
            Self::CfgPrunedMacroInput => "cfg_pruned_macro_input",
        }
    }
}

const GRAPH_LINT_FALSE_POSITIVE_BOUNDARY: &str = "lints inspect only StableGraphMetadata; runtime-only policy, external guard conditions, and transition sites rejected before metadata emission are outside this report.";

impl StableGraphMetadata {
    /// Renders this stable graph metadata as a Mermaid state diagram.
    ///
    /// The diagram is generated only from this metadata document. It does not
    /// re-inspect source code or claim stronger semantic authority than the
    /// metadata's recorded `authority` field.
    pub fn to_mermaid_state_diagram(&self) -> String {
        let mut output = String::from("stateDiagram-v2\n");
        output.push_str("    %% machine: ");
        output.push_str(&escape_mermaid_comment(&self.machine.rust_type_path));
        output.push('\n');

        for (index, state) in self.states.iter().enumerate() {
            output.push_str("    state \"");
            output.push_str(&escape_mermaid_label(
                state.label.as_deref().unwrap_or(&state.rust_name),
            ));
            output.push_str("\" as s");
            output.push_str(&index.to_string());
            output.push('\n');
        }

        let mut unknown_states: Vec<&str> = Vec::new();
        for transition in &self.transitions {
            let Some(from_index) = self.state_index(&transition.from_state) else {
                continue;
            };

            for target in &transition.to_states {
                output.push_str("    s");
                output.push_str(&from_index.to_string());
                output.push_str(" --> ");
                match self.state_index(target) {
                    Some(target_index) => {
                        output.push('s');
                        output.push_str(&target_index.to_string());
                    }
                    None => {
                        let unknown_index = unknown_state_index(&mut unknown_states, target);
                        output.push_str("unknown_");
                        output.push_str(&unknown_index.to_string());
                    }
                }
                output.push_str(": ");
                output.push_str(&escape_mermaid_edge_label(
                    transition
                        .label
                        .as_deref()
                        .unwrap_or(&transition.method_name),
                ));
                output.push('\n');
            }
        }

        for (index, state_name) in unknown_states.iter().enumerate() {
            output.push_str("    state \"");
            output.push_str(&escape_mermaid_label(state_name));
            output.push_str("\" as unknown_");
            output.push_str(&index.to_string());
            output.push('\n');
        }

        output
    }

    /// Renders this stable graph metadata as a Graphviz DOT directed graph.
    ///
    /// The DOT document is emitted from graph-order metadata with stable node ids
    /// (`s0`, `s1`, ...) so repeated runs over the same metadata produce byte-for-
    /// byte identical output. It does not re-inspect source code or claim stronger
    /// semantic authority than the metadata's recorded `authority` field.
    pub fn to_dot_graph(&self) -> String {
        let mut output = String::from("digraph statum_workflow {\n");
        output.push_str("    graph [label=\"");
        output.push_str(&escape_dot_string(&self.machine.rust_type_path));
        output.push_str("\", labelloc=\"t\"];\n");
        output.push_str("    node [shape=\"box\"];\n");

        for (index, state) in self.states.iter().enumerate() {
            output.push_str("    s");
            output.push_str(&index.to_string());
            output.push_str(" [label=\"");
            output.push_str(&escape_dot_string(
                state.label.as_deref().unwrap_or(&state.rust_name),
            ));
            output.push_str("\"];\n");
        }

        let mut unknown_states: Vec<&str> = Vec::new();
        for transition in &self.transitions {
            let Some(from_index) = self.state_index(&transition.from_state) else {
                continue;
            };

            for target in &transition.to_states {
                output.push_str("    s");
                output.push_str(&from_index.to_string());
                output.push_str(" -> ");
                match self.state_index(target) {
                    Some(target_index) => {
                        output.push('s');
                        output.push_str(&target_index.to_string());
                    }
                    None => {
                        let unknown_index = unknown_state_index(&mut unknown_states, target);
                        output.push_str("unknown_");
                        output.push_str(&unknown_index.to_string());
                    }
                }
                output.push_str(" [label=\"");
                output.push_str(&escape_dot_string(
                    transition
                        .label
                        .as_deref()
                        .unwrap_or(&transition.method_name),
                ));
                output.push_str("\"];\n");
            }
        }

        for (index, state_name) in unknown_states.iter().enumerate() {
            output.push_str("    unknown_");
            output.push_str(&index.to_string());
            output.push_str(" [label=\"");
            output.push_str(&escape_dot_string(state_name));
            output.push_str("\"];\n");
        }

        output.push_str("}\n");
        output
    }

    /// Renders this stable graph metadata as a Markdown transition matrix.
    ///
    /// Rows are source states, columns are target states, and each cell is either
    /// a comma-separated list of transition labels/methods allowed from that
    /// source to that target or the literal `forbidden`. The table is generated
    /// only from this metadata document and claims no stronger authority than the
    /// metadata's recorded `authority` field.
    pub fn to_transition_matrix_table(&self) -> String {
        let mut cells = vec![
            vec![Vec::<&StableTransitionMetadata>::new(); self.states.len()];
            self.states.len()
        ];

        for transition in &self.transitions {
            let Some(from_index) = self.state_index(&transition.from_state) else {
                continue;
            };

            for target in &transition.to_states {
                if let Some(target_index) = self.state_index(target) {
                    cells[from_index][target_index].push(transition);
                }
            }
        }

        let mut output = String::new();
        output.push_str("| from \\ to |");
        for state in &self.states {
            output.push(' ');
            output.push_str(&escape_markdown_table_cell(
                state.label.as_deref().unwrap_or(&state.rust_name),
            ));
            output.push_str(" |");
        }
        output.push('\n');

        output.push_str("| --- |");
        for _ in &self.states {
            output.push_str(" --- |");
        }
        output.push('\n');

        for (from_index, state) in self.states.iter().enumerate() {
            output.push_str("| ");
            output.push_str(&escape_markdown_table_cell(
                state.label.as_deref().unwrap_or(&state.rust_name),
            ));
            output.push_str(" |");

            for target_cell in &cells[from_index] {
                output.push(' ');
                if target_cell.is_empty() {
                    output.push_str("forbidden");
                } else {
                    let labels = target_cell
                        .iter()
                        .map(|transition| {
                            escape_markdown_table_cell(
                                transition
                                    .label
                                    .as_deref()
                                    .unwrap_or(&transition.method_name),
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    output.push_str(&labels);
                }
                output.push_str(" |");
            }
            output.push('\n');
        }

        output
    }

    /// Lints this stable graph metadata for structural invariant smells.
    ///
    /// These checks observe only this metadata document. They intentionally use
    /// conservative graph-order and naming heuristics for prototype tooling, so
    /// findings are warnings and may be false positives when runtime policy or
    /// unsupported macro shapes carry stronger semantics than the exported graph.
    pub fn lint_graph_invariants(&self) -> Vec<GraphLintFinding> {
        let mut findings = Vec::new();
        let state_names = self
            .states
            .iter()
            .map(|state| state.rust_name.as_str())
            .collect::<Vec<_>>();

        for transition in &self.transitions {
            if terminal_like_state_name(&transition.from_state) {
                findings.push(GraphLintFinding {
                    code: GraphLintCode::TerminalStateHasOutgoingTransition,
                    message: format!(
                        "state `{}` has outgoing transition `{}`; terminal-state lint treats terminal-looking state names as terminal candidates.",
                        transition.from_state, transition.method_name
                    ),
                    state: Some(transition.from_state.clone()),
                    transition: Some(transition.method_name.clone()),
                });
            }

            if transition
                .to_states
                .iter()
                .any(|target| target == &transition.from_state)
            {
                findings.push(GraphLintFinding {
                    code: GraphLintCode::SuspiciousSelfTransition,
                    message: format!(
                        "transition `{}` can return from `{}` to itself; verify the loop is intentional.",
                        transition.method_name, transition.from_state
                    ),
                    state: Some(transition.from_state.clone()),
                    transition: Some(transition.method_name.clone()),
                });
            }
        }

        for state_name in state_names.iter().skip(1) {
            if !self.transitions.iter().any(|transition| {
                transition
                    .to_states
                    .iter()
                    .any(|target| target == state_name)
            }) {
                findings.push(GraphLintFinding {
                    code: GraphLintCode::MissingIncomingPath,
                    message: format!(
                        "state `{state_name}` has no incoming exported transition; graph-order lint treats the first state as the root."
                    ),
                    state: Some((*state_name).to_owned()),
                    transition: None,
                });
            }
        }

        let reachable = self.reachable_state_names_from_first_state();
        for state_name in state_names.iter().skip(1) {
            if !reachable.iter().any(|reachable| reachable == state_name) {
                findings.push(GraphLintFinding {
                    code: GraphLintCode::UnreachableState,
                    message: format!(
                        "state `{state_name}` is not reachable from the first exported state through exported transitions."
                    ),
                    state: Some((*state_name).to_owned()),
                    transition: None,
                });
            }
        }

        findings
    }

    /// Renders graph invariant lint findings as deterministic text for CLI and CI use.
    pub fn to_graph_lint_report(&self) -> String {
        let findings = self.lint_graph_invariants();
        let mut output = String::new();
        output.push_str("Graph invariant lint report for ");
        output.push_str(&escape_plain_text_report_field(
            &self.machine.rust_type_path,
        ));
        output.push('\n');
        output.push_str("authority: ");
        output.push_str(self.authority.as_str());
        output.push('\n');
        output.push_str("false-positive boundary: ");
        output.push_str(GRAPH_LINT_FALSE_POSITIVE_BOUNDARY);
        output.push_str("\n\n");

        if findings.is_empty() {
            output.push_str("No graph invariant warnings.\n");
            return output;
        }

        for finding in findings {
            output.push_str("- ");
            output.push_str(finding.code.as_str());
            output.push_str(": ");
            output.push_str(&escape_plain_text_report_field(&finding.message));
            output.push('\n');
        }

        output
    }

    fn reachable_state_names_from_first_state(&self) -> Vec<&str> {
        let Some(first_state) = self.states.first() else {
            return Vec::new();
        };
        let mut reachable = vec![first_state.rust_name.as_str()];
        let mut cursor = 0;

        while cursor < reachable.len() {
            let source = reachable[cursor];
            cursor += 1;
            for transition in self
                .transitions
                .iter()
                .filter(|transition| transition.from_state == source)
            {
                for target in &transition.to_states {
                    if self.state_index(target).is_some()
                        && !reachable.iter().any(|reachable| *reachable == target)
                    {
                        reachable.push(target);
                    }
                }
            }
        }

        reachable
    }

    fn state_index(&self, rust_name: &str) -> Option<usize> {
        self.states
            .iter()
            .position(|state| state.rust_name == rust_name)
    }

    /// Builds the stable external metadata document from Statum's typed graph.
    ///
    /// The conversion intentionally lowers typed ids into Rust names so the JSON
    /// shape is stable for tooling. It does not inspect function bodies, runtime
    /// values, or expanded Rust items outside the macro input that produced
    /// `graph`.
    pub fn from_graph<S, T>(graph: &MachineGraph<S, T>) -> Self
    where
        S: Copy + Eq + 'static,
        T: Copy + Eq + 'static,
    {
        let states = graph
            .states
            .iter()
            .map(|state| StableStateMetadata {
                rust_name: state.rust_name.to_owned(),
                label: None,
                description: None,
                has_data: state.has_data,
                fields: Vec::new(),
            })
            .collect();

        let transitions = graph
            .transitions
            .iter()
            .map(|transition| StableTransitionMetadata {
                method_name: transition.method_name.to_owned(),
                label: None,
                description: None,
                from_state: graph
                    .state(transition.from)
                    .map(|state| state.rust_name)
                    .unwrap_or("<unknown>")
                    .to_owned(),
                to_states: transition
                    .to
                    .iter()
                    .map(|target| {
                        graph
                            .state(*target)
                            .map(|state| state.rust_name)
                            .unwrap_or("<unknown>")
                            .to_owned()
                    })
                    .collect(),
            })
            .collect();

        Self {
            version: StableGraphMetadataVersion::V1,
            authority: GraphAuthorityLevel::CfgPrunedMacroInput,
            unsupported_cases: vec![
                UnsupportedGraphMetadataCase::RuntimeOnlyTransitions,
                UnsupportedGraphMetadataCase::CfgAmbiguousAliases,
                UnsupportedGraphMetadataCase::UnexpandedCustomDecisionEnums,
                UnsupportedGraphMetadataCase::MacroGeneratedItems,
                UnsupportedGraphMetadataCase::IncludeGeneratedItems,
                UnsupportedGraphMetadataCase::FieldLevelPresentationMetadata,
            ],
            machine: StableMachineMetadata {
                module_path: graph.machine.module_path.to_owned(),
                rust_type_path: graph.machine.rust_type_path.to_owned(),
                label: None,
                description: None,
                fields: Vec::new(),
            },
            states,
            transitions,
        }
    }
}

fn escape_plain_text_report_field(value: &str) -> String {
    value.replace(['\n', '\r', '\t'], " ")
}

fn terminal_like_state_name(state_name: &str) -> bool {
    let normalized = state_name.to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "complete" | "completed" | "done" | "final" | "terminal" | "published" | "archived"
    ) || normalized.ends_with("complete")
        || normalized.ends_with("completed")
        || normalized.ends_with("done")
        || normalized.ends_with("published")
        || normalized.ends_with("archived")
}

fn unknown_state_index<'a>(unknown_states: &mut Vec<&'a str>, state_name: &'a str) -> usize {
    match unknown_states
        .iter()
        .position(|unknown_state| *unknown_state == state_name)
    {
        Some(index) => index,
        None => {
            unknown_states.push(state_name);
            unknown_states.len() - 1
        }
    }
}

fn escape_mermaid_comment(value: &str) -> String {
    value.replace(['\n', '\r'], " ")
}

fn escape_mermaid_edge_label(value: &str) -> String {
    value.replace(['\n', '\r'], " ")
}

fn escape_mermaid_label(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' | '\r' => escaped.push(' '),
            other => escaped.push(other),
        }
    }
    escaped
}

fn escape_dot_string(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' | '\r' => escaped.push_str("\\n"),
            other => escaped.push(other),
        }
    }
    escaped
}

fn escape_markdown_table_cell(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '|' => escaped.push_str("\\|"),
            '\n' | '\r' => escaped.push(' '),
            other => escaped.push(other),
        }
    }
    escaped
}

/// Stable machine-level metadata.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct StableMachineMetadata {
    /// `module_path!()` for the source module that owns the machine.
    pub module_path: String,
    /// Fully qualified Rust type path for the machine family.
    pub rust_type_path: String,
    /// Optional human-facing label when a presentation layer supplies one.
    pub label: Option<String>,
    /// Optional human-facing description when a presentation layer supplies one.
    pub description: Option<String>,
    /// Reserved field metadata. Current graph emission leaves this empty.
    pub fields: Vec<StableFieldMetadata>,
}

/// Stable state-level metadata.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct StableStateMetadata {
    /// Rust marker/variant name for the state.
    pub rust_name: String,
    /// Optional human-facing label when a presentation layer supplies one.
    pub label: Option<String>,
    /// Optional human-facing description when a presentation layer supplies one.
    pub description: Option<String>,
    /// Whether the state carries `state_data`.
    pub has_data: bool,
    /// Reserved state payload field metadata. Current graph emission leaves this
    /// empty because field-level presentation metadata is unsupported.
    pub fields: Vec<StableFieldMetadata>,
}

/// Stable transition-site metadata.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct StableTransitionMetadata {
    /// Rust method name for the transition site.
    pub method_name: String,
    /// Optional human-facing label when a presentation layer supplies one.
    pub label: Option<String>,
    /// Optional human-facing description when a presentation layer supplies one.
    pub description: Option<String>,
    /// Rust state name of the exact source state.
    pub from_state: String,
    /// Rust state names of the exact legal target states.
    pub to_states: Vec<String>,
}

/// Reserved stable field metadata shape.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct StableFieldMetadata {
    /// Rust field name when available.
    pub rust_name: String,
    /// Optional human-facing label when a presentation layer supplies one.
    pub label: Option<String>,
    /// Optional human-facing description when a presentation layer supplies one.
    pub description: Option<String>,
}
