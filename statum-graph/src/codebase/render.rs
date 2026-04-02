use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::codebase::{
    CodebaseDoc, CodebaseMachine, CodebaseRelationDetail, CodebaseRelationSource, CodebaseState,
};
use crate::render::{bundle_output_path, validate_output_stem};

/// Error returned when a selected codebase diagram subject is missing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagramError {
    /// One selected machine index does not exist in the codebase document.
    MissingMachine { index: usize },
    /// One selected relation index does not exist in the codebase document.
    MissingRelation { index: usize },
    /// One selected machine is not a composition machine, so exact journey
    /// enumeration is unavailable.
    NotCompositionMachine { index: usize },
    /// One selected composition machine has no graph-root state in the exact
    /// linked surface, so finite journey enumeration is unavailable.
    MissingJourneyRoot { index: usize },
    /// One selected composition machine has a reachable cycle, so finite exact
    /// journey enumeration is unavailable.
    ReachableJourneyCycle { index: usize },
    /// One selected composition machine exceeds the exact journey enumeration
    /// budget, so the renderer fails closed instead of exporting a partial
    /// journey set.
    TooManyJourneys { index: usize },
    /// One selected journey id is missing from the enumerated journey set for
    /// the selected machine.
    MissingJourney { machine_index: usize },
}

impl std::fmt::Display for DiagramError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingMachine { index } => {
                write!(formatter, "codebase machine index {index} is missing")
            }
            Self::MissingRelation { index } => {
                write!(formatter, "codebase relation index {index} is missing")
            }
            Self::NotCompositionMachine { index } => write!(
                formatter,
                "codebase machine index {index} is not a composition machine"
            ),
            Self::MissingJourneyRoot { index } => write!(
                formatter,
                "codebase machine index {index} has no graph-root state for exact journey rendering"
            ),
            Self::ReachableJourneyCycle { index } => write!(
                formatter,
                "codebase machine index {index} has a reachable cycle, so finite exact journeys are unavailable"
            ),
            Self::TooManyJourneys { index } => write!(
                formatter,
                "codebase machine index {index} exceeds the exact journey enumeration budget"
            ),
            Self::MissingJourney { machine_index } => write!(
                formatter,
                "selected journey is missing from codebase machine index {machine_index}"
            ),
        }
    }
}

impl std::error::Error for DiagramError {}

/// Maximum number of exact machine journeys rendered directly before the
/// inspector switches into grouped-family mode.
pub const MAX_DIRECT_JOURNEYS: usize = 64;

/// Maximum number of exact journeys enumerated for one composition machine.
pub const MAX_EXACT_JOURNEYS: usize = 256;

/// Maximum depth-first-search expansions allowed while enumerating exact
/// journeys for one composition machine.
pub const MAX_DFS_EXPANSIONS: usize = 16_384;

/// One exact journey step within one composition machine.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct JourneyStep {
    /// The selected transition index from the source state.
    pub transition: usize,
    /// The selected target state index for that transition.
    pub to_state: usize,
}

/// Snapshot-scoped semantic identity for one exact journey.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct JourneyId {
    /// The graph-root ingress state.
    pub ingress_state: usize,
    /// The ordered exact transition sequence for the selected journey.
    pub steps: Vec<JourneyStep>,
}

/// One exact finite root-to-sink journey for one composition machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Journey {
    /// Snapshot-scoped journey identity.
    pub id: JourneyId,
    /// Sink state reached by the selected journey.
    pub egress_state: usize,
}

impl Journey {
    /// Graph-root ingress state for the selected journey.
    pub fn ingress_state(&self) -> usize {
        self.id.ingress_state
    }

    /// Ordered exact steps for the selected journey.
    pub fn steps(&self) -> &[JourneyStep] {
        &self.id.steps
    }
}

/// One built-in renderer output format for codebase documents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    Mermaid,
    Dot,
    PlantUml,
    Json,
}

/// Orientation for Mermaid workspace flowchart renderers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceFlowDirection {
    TopDown,
    LeftRight,
}

impl WorkspaceFlowDirection {
    const fn mermaid_keyword(self) -> &'static str {
        match self {
            Self::TopDown => "TD",
            Self::LeftRight => "LR",
        }
    }
}

/// Label density for Mermaid workspace flowchart edges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceFlowEdgeLabelMode {
    Full,
    Compact,
    Hidden,
}

/// Exact workspace-flow projection options for Mermaid machine-level diagrams.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkspaceFlowOptions<'a> {
    /// Machine indices to include in the projection. `None` means every machine.
    pub machine_indices: Option<&'a [usize]>,
    /// Mermaid graph orientation.
    pub direction: WorkspaceFlowDirection,
    /// Whether to prefer shorter machine labels.
    pub compact_labels: bool,
    /// Whether to prefer shorter grouped edge labels or hide them entirely.
    pub edge_labels: WorkspaceFlowEdgeLabelMode,
    /// Whether to express machine role through Mermaid node shape.
    pub role_shapes: bool,
}

impl Default for WorkspaceFlowOptions<'_> {
    fn default() -> Self {
        Self {
            machine_indices: None,
            direction: WorkspaceFlowDirection::TopDown,
            compact_labels: false,
            edge_labels: WorkspaceFlowEdgeLabelMode::Full,
            role_shapes: false,
        }
    }
}

impl Format {
    /// All built-in renderer formats in stable bundle order.
    pub const ALL: [Self; 4] = [Self::Mermaid, Self::Dot, Self::PlantUml, Self::Json];

    /// Conventional file extension for this format.
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Mermaid => "mmd",
            Self::Dot => "dot",
            Self::PlantUml => "puml",
            Self::Json => "json",
        }
    }

    /// Renders one codebase document into this format.
    pub fn render(self, doc: &CodebaseDoc) -> String {
        match self {
            Self::Mermaid => mermaid(doc),
            Self::Dot => dot(doc),
            Self::PlantUml => plantuml(doc),
            Self::Json => json(doc),
        }
    }

    /// Renders one codebase document and writes it to one filesystem path.
    pub fn write_to<P>(self, doc: &CodebaseDoc, path: P) -> io::Result<PathBuf>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        ensure_parent_dir(path)?;
        fs::write(path, self.render(doc))?;
        Ok(path.to_path_buf())
    }
}

/// Renders one codebase document into every built-in format and writes the
/// resulting files into `dir` using `stem` plus the format extension.
pub fn write_all_to_dir<P>(doc: &CodebaseDoc, dir: P, stem: &str) -> io::Result<Vec<PathBuf>>
where
    P: AsRef<Path>,
{
    let dir = dir.as_ref();
    validate_output_stem(stem)?;
    fs::create_dir_all(dir)?;

    Format::ALL
        .into_iter()
        .map(|format| {
            bundle_output_path(dir, stem, format.extension())
                .and_then(|path| format.write_to(doc, path))
        })
        .collect()
}

/// Renders one codebase machine as Mermaid state diagram text.
pub fn mermaid_machine_state(
    doc: &CodebaseDoc,
    machine_index: usize,
) -> Result<String, DiagramError> {
    let machine = doc
        .machine(machine_index)
        .ok_or(DiagramError::MissingMachine {
            index: machine_index,
        })?;
    Ok(render_machine_state_diagram(machine))
}

/// Enumerates every exact finite root-to-sink journey for one selected
/// composition machine.
pub fn machine_journeys(
    doc: &CodebaseDoc,
    machine_index: usize,
) -> Result<Vec<Journey>, DiagramError> {
    let machine = doc
        .machine(machine_index)
        .ok_or(DiagramError::MissingMachine {
            index: machine_index,
        })?;
    machine_journeys_for_machine(machine)
}

/// Renders one exact composition-machine journey as Mermaid state diagram text.
pub fn mermaid_machine_journey(
    doc: &CodebaseDoc,
    machine_index: usize,
    journey_id: &JourneyId,
) -> Result<String, DiagramError> {
    let machine = doc
        .machine(machine_index)
        .ok_or(DiagramError::MissingMachine {
            index: machine_index,
        })?;
    let journeys = machine_journeys_for_machine(machine)?;
    let journey = journeys
        .iter()
        .find(|journey| journey.id == *journey_id)
        .ok_or(DiagramError::MissingJourney { machine_index })?;
    Ok(render_machine_journey_diagram(machine, journey))
}

/// Renders one exact relation as Mermaid sequence diagram text.
pub fn mermaid_relation_sequence(
    doc: &CodebaseDoc,
    relation_index: usize,
) -> Result<String, DiagramError> {
    let detail = doc
        .relation_detail(relation_index)
        .ok_or(DiagramError::MissingRelation {
            index: relation_index,
        })?;
    Ok(render_relation_sequence(detail))
}

/// Renders a combined linked-machine topology as Mermaid flowchart text.
pub fn mermaid(doc: &CodebaseDoc) -> String {
    let mut lines = vec![
        format!("%% linked machines: {}", doc.machines().len()),
        "graph TD".to_string(),
    ];
    let relation_groups = cross_machine_relation_groups(doc);
    let has_validator_entries = doc
        .machines()
        .iter()
        .any(|machine| !machine.validator_entries.is_empty());

    for machine in doc.machines() {
        lines.push(format!(
            "    subgraph {}[\"{}\"]",
            machine.cluster_id(),
            escape_mermaid_label(&render_machine_cluster_label(machine))
        ));
        for state in &machine.states {
            lines.push(format!(
                "        {}[\"{}\"]",
                machine.node_id(state.index),
                escape_mermaid_label(&render_state_label(state))
            ));
        }
        lines.push("    end".to_string());
    }

    if has_validator_entries && !doc.machines().is_empty() {
        lines.push(String::new());
    }

    for machine in doc.machines() {
        for entry in &machine.validator_entries {
            lines.push(format!(
                "    {}(\"{}\")",
                machine.validator_node_id(entry.index),
                escape_mermaid_label(&entry.display_label())
            ));
        }
    }

    if !doc.machines().is_empty() && (has_validator_entries || any_transitions(doc)) {
        lines.push(String::new());
    }

    for machine in doc.machines() {
        for transition in &machine.transitions {
            let from = machine.node_id(transition.from);
            for target in &transition.to {
                let to = machine.node_id(*target);
                lines.push(format!(
                    "    {from} -->|{}| {to}",
                    escape_mermaid_edge_label(transition.display_label())
                ));
            }
        }
    }

    if !relation_groups.is_empty() && !doc.machines().is_empty() {
        lines.push(String::new());
    }

    for group in &relation_groups {
        let from_machine = doc
            .machine(group.from_machine)
            .expect("relation group source machine should exist");
        let to_machine = doc
            .machine(group.to_machine)
            .expect("relation group target machine should exist");
        lines.push(format!(
            "    {} ==>|{}| {}",
            from_machine.cluster_id(),
            escape_mermaid_edge_label(&group.display_label()),
            to_machine.cluster_id()
        ));
    }

    if !doc.links().is_empty() && (!doc.machines().is_empty() || !relation_groups.is_empty()) {
        lines.push(String::new());
    }

    for link in doc.links() {
        let from_machine = doc
            .machine(link.from_machine)
            .expect("codebase link source machine should exist");
        let to_machine = doc
            .machine(link.to_machine)
            .expect("codebase link target machine should exist");
        lines.push(format!(
            "    {} -.->|{}| {}",
            from_machine.node_id(link.from_state),
            escape_mermaid_edge_label(link.display_label()),
            to_machine.node_id(link.to_state)
        ));
    }

    if has_validator_entries
        && (!doc.links().is_empty() || any_transitions(doc) || !doc.machines().is_empty())
    {
        lines.push(String::new());
    }

    for machine in doc.machines() {
        for entry in &machine.validator_entries {
            let from = machine.validator_node_id(entry.index);
            for target in &entry.target_states {
                lines.push(format!("    {from} -.-> {}", machine.node_id(*target)));
            }
        }
    }

    lines.join("\n")
}

/// Renders an exact machine-level workspace flow projection as Mermaid flowchart
/// text.
pub fn mermaid_workspace_flow(
    doc: &CodebaseDoc,
    options: WorkspaceFlowOptions<'_>,
) -> Result<String, DiagramError> {
    let machine_indices = selected_machine_indices(doc, options.machine_indices)?;
    let machine_labels = workspace_machine_labels(doc, &machine_indices, options.compact_labels);
    let relation_groups = cross_machine_relation_groups(doc)
        .into_iter()
        .filter(|group| {
            machine_indices.contains(&group.from_machine)
                && machine_indices.contains(&group.to_machine)
        })
        .collect::<Vec<_>>();
    let link_groups = cross_machine_link_groups(doc)
        .into_iter()
        .filter(|group| {
            machine_indices.contains(&group.from_machine)
                && machine_indices.contains(&group.to_machine)
        })
        .collect::<Vec<_>>();

    let mut lines = vec![
        format!(
            "%% workspace machines shown: {} of {}",
            machine_indices.len(),
            doc.machines().len()
        ),
        format!("graph {}", options.direction.mermaid_keyword()),
    ];

    for machine_index in &machine_indices {
        let machine = doc
            .machine(*machine_index)
            .expect("selected workspace-flow machine should exist");
        let label = machine_labels
            .get(machine_index)
            .expect("selected workspace-flow machine label should exist");
        lines.push(render_workspace_machine_node(
            machine,
            label,
            options.role_shapes,
        ));
    }

    if !relation_groups.is_empty() && !machine_indices.is_empty() {
        lines.push(String::new());
    }

    for group in relation_groups {
        let from_machine = doc
            .machine(group.from_machine)
            .expect("workspace-flow relation source machine should exist");
        let to_machine = doc
            .machine(group.to_machine)
            .expect("workspace-flow relation target machine should exist");
        lines.push(render_workspace_flow_edge(
            from_machine.cluster_id(),
            workspace_relation_arrow(group.semantic),
            &render_workspace_relation_group_label(&group, options.edge_labels),
            to_machine.cluster_id(),
        ));
    }

    if !link_groups.is_empty() && (!machine_indices.is_empty() || !lines.is_empty()) {
        lines.push(String::new());
    }

    for group in link_groups {
        let from_machine = doc
            .machine(group.from_machine)
            .expect("workspace-flow link source machine should exist");
        let to_machine = doc
            .machine(group.to_machine)
            .expect("workspace-flow link target machine should exist");
        lines.push(render_workspace_flow_edge(
            from_machine.cluster_id(),
            "-.->",
            &render_workspace_link_group_label(&group, options.edge_labels),
            to_machine.cluster_id(),
        ));
    }

    Ok(lines.join("\n"))
}

/// Renders a combined linked-machine topology as DOT text.
pub fn dot(doc: &CodebaseDoc) -> String {
    let mut lines = vec![
        "digraph \"statum_codebase\" {".to_string(),
        "    rankdir=TB;".to_string(),
    ];
    let relation_groups = cross_machine_relation_groups(doc);
    let has_validator_entries = doc
        .machines()
        .iter()
        .any(|machine| !machine.validator_entries.is_empty());

    for machine in doc.machines() {
        lines.push(format!(
            "    subgraph \"cluster_{}\" {{",
            machine.cluster_id()
        ));
        lines.push(format!(
            "        label=\"{}\";",
            escape_dot_label(&render_machine_cluster_label(machine))
        ));
        for state in &machine.states {
            lines.push(format!(
                "        {} [label=\"{}\"]",
                machine.node_id(state.index),
                escape_dot_label(&render_state_label(state))
            ));
        }
        lines.push(format!(
            "        {} [label=\"\", shape=point, width=0.01, height=0.01, style=invis]",
            machine.summary_node_id()
        ));
        lines.push("    }".to_string());
    }

    if has_validator_entries && !doc.machines().is_empty() {
        lines.push(String::new());
    }

    for machine in doc.machines() {
        for entry in &machine.validator_entries {
            lines.push(format!(
                "    {} [label=\"{}\", shape=ellipse, style=\"rounded,dashed\", color=\"#4b5563\"]",
                machine.validator_node_id(entry.index),
                escape_dot_label(&entry.display_label())
            ));
        }
    }

    if !doc.machines().is_empty() && (has_validator_entries || any_transitions(doc)) {
        lines.push(String::new());
    }

    for machine in doc.machines() {
        for transition in &machine.transitions {
            let from = machine.node_id(transition.from);
            for target in &transition.to {
                let to = machine.node_id(*target);
                lines.push(format!(
                    "    {from} -> {to} [label=\"{}\"]",
                    escape_dot_label(transition.display_label())
                ));
            }
        }
    }

    if !relation_groups.is_empty() && !doc.machines().is_empty() {
        lines.push(String::new());
    }

    for group in &relation_groups {
        let from_machine = doc
            .machine(group.from_machine)
            .expect("relation group source machine should exist");
        let to_machine = doc
            .machine(group.to_machine)
            .expect("relation group target machine should exist");
        lines.push(format!(
            "    {} -> {} [ltail=\"cluster_{}\", lhead=\"cluster_{}\", style=\"bold,dotted\", color=\"#2563eb\", fontcolor=\"#2563eb\", penwidth=2, minlen=2, label=\"{}\"]",
            from_machine.summary_node_id(),
            to_machine.summary_node_id(),
            from_machine.cluster_id(),
            to_machine.cluster_id(),
            escape_dot_label(&group.display_label())
        ));
    }

    if !doc.links().is_empty() && (!doc.machines().is_empty() || !relation_groups.is_empty()) {
        lines.push(String::new());
    }

    for link in doc.links() {
        let from_machine = doc
            .machine(link.from_machine)
            .expect("codebase link source machine should exist");
        let to_machine = doc
            .machine(link.to_machine)
            .expect("codebase link target machine should exist");
        lines.push(format!(
            "    {} -> {} [style=dashed, label=\"{}\"]",
            from_machine.node_id(link.from_state),
            to_machine.node_id(link.to_state),
            escape_dot_label(link.display_label())
        ));
    }

    if has_validator_entries
        && (!doc.links().is_empty() || any_transitions(doc) || !doc.machines().is_empty())
    {
        lines.push(String::new());
    }

    for machine in doc.machines() {
        for entry in &machine.validator_entries {
            let from = machine.validator_node_id(entry.index);
            for target in &entry.target_states {
                lines.push(format!(
                    "    {from} -> {} [style=dashed, color=\"#4b5563\", penwidth=2, constraint=false]",
                    machine.node_id(*target)
                ));
            }
        }
    }

    lines.push("}".to_string());
    lines.join("\n")
}

/// Renders a combined linked-machine topology as PlantUML state text.
pub fn plantuml(doc: &CodebaseDoc) -> String {
    let mut lines = vec![
        "@startuml".to_string(),
        format!("' linked machines: {}", doc.machines().len()),
    ];
    let relation_groups = cross_machine_relation_groups(doc);
    let has_validator_entries = doc
        .machines()
        .iter()
        .any(|machine| !machine.validator_entries.is_empty());

    for machine in doc.machines() {
        lines.push(format!(
            "state \"{}\" as {} {{",
            escape_plantuml_label(&render_machine_cluster_label(machine)),
            machine.cluster_id()
        ));
        for state in &machine.states {
            lines.push(format!(
                "    state \"{}\" as {}",
                escape_plantuml_label(&render_state_label(state)),
                machine.node_id(state.index)
            ));
        }
        lines.push("}".to_string());
    }

    if has_validator_entries && !doc.machines().is_empty() {
        lines.push(String::new());
    }

    for machine in doc.machines() {
        for entry in &machine.validator_entries {
            lines.push(format!(
                "state \"{}\" as {} <<validator-entry>>",
                escape_plantuml_label(&entry.display_label()),
                machine.validator_node_id(entry.index)
            ));
        }
    }

    if !doc.machines().is_empty() && (has_validator_entries || any_transitions(doc)) {
        lines.push(String::new());
    }

    for machine in doc.machines() {
        for transition in &machine.transitions {
            let from = machine.node_id(transition.from);
            for target in &transition.to {
                let to = machine.node_id(*target);
                lines.push(format!(
                    "{from} --> {to} : {}",
                    escape_plantuml_label(transition.display_label())
                ));
            }
        }
    }

    if !relation_groups.is_empty() && !doc.machines().is_empty() {
        lines.push(String::new());
    }

    for group in &relation_groups {
        let from_machine = doc
            .machine(group.from_machine)
            .expect("relation group source machine should exist");
        let to_machine = doc
            .machine(group.to_machine)
            .expect("relation group target machine should exist");
        lines.push(format!(
            "{} -[#2563EB,bold]-> {} : {}",
            from_machine.cluster_id(),
            to_machine.cluster_id(),
            escape_plantuml_label(&group.display_label())
        ));
    }

    if !doc.links().is_empty() && (!doc.machines().is_empty() || !relation_groups.is_empty()) {
        lines.push(String::new());
    }

    for link in doc.links() {
        let from_machine = doc
            .machine(link.from_machine)
            .expect("codebase link source machine should exist");
        let to_machine = doc
            .machine(link.to_machine)
            .expect("codebase link target machine should exist");
        lines.push(format!(
            "{} ..> {} : {}",
            from_machine.node_id(link.from_state),
            to_machine.node_id(link.to_state),
            escape_plantuml_label(link.display_label())
        ));
    }

    if has_validator_entries
        && (!doc.links().is_empty() || any_transitions(doc) || !doc.machines().is_empty())
    {
        lines.push(String::new());
    }

    for machine in doc.machines() {
        for entry in &machine.validator_entries {
            let from = machine.validator_node_id(entry.index);
            for target in &entry.target_states {
                lines.push(format!(
                    "{from} ..> {} : validator entry",
                    machine.node_id(*target)
                ));
            }
        }
    }

    lines.push("@enduml".to_string());
    lines.join("\n")
}

/// Renders a combined linked-machine topology as deterministic pretty JSON.
pub fn json(doc: &CodebaseDoc) -> String {
    serde_json::to_string_pretty(doc).expect("CodebaseDoc serialization should not fail")
}

fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }

    Ok(())
}

fn any_transitions(doc: &CodebaseDoc) -> bool {
    doc.machines()
        .iter()
        .any(|machine| !machine.transitions.is_empty())
}

#[derive(Debug, Default)]
struct JourneyEnumerationBudget {
    expansions: usize,
    exceeded: bool,
}

/// Enumerates every exact finite root-to-sink journey for one selected
/// composition machine already resolved from the linked codebase.
pub fn machine_journeys_for_machine(
    machine: &CodebaseMachine,
) -> Result<Vec<Journey>, DiagramError> {
    if !machine.role.is_composition() {
        return Err(DiagramError::NotCompositionMachine {
            index: machine.index,
        });
    }

    let roots = machine
        .states
        .iter()
        .filter(|state| state.is_graph_root)
        .map(|state| state.index)
        .collect::<Vec<_>>();
    if roots.is_empty() {
        return Err(DiagramError::MissingJourneyRoot {
            index: machine.index,
        });
    }

    let mut outgoing = vec![Vec::<JourneyStep>::new(); machine.states.len()];
    for transition in &machine.transitions {
        for &to_state in &transition.to {
            outgoing[transition.from].push(JourneyStep {
                transition: transition.index,
                to_state,
            });
        }
    }
    for steps in &mut outgoing {
        steps.sort();
    }

    let mut cycle_detected = false;
    let mut budget = JourneyEnumerationBudget::default();
    let mut journeys = Vec::new();
    for root in roots {
        let mut state_stack = BTreeSet::new();
        let mut steps = Vec::new();
        enumerate_machine_journeys_from_state(
            root,
            root,
            &outgoing,
            &mut state_stack,
            &mut steps,
            &mut journeys,
            &mut cycle_detected,
            &mut budget,
        );
        if cycle_detected || budget.exceeded {
            break;
        }
    }

    if cycle_detected {
        return Err(DiagramError::ReachableJourneyCycle {
            index: machine.index,
        });
    }
    if budget.exceeded {
        return Err(DiagramError::TooManyJourneys {
            index: machine.index,
        });
    }

    journeys.sort_by(|left, right| {
        left.ingress_state()
            .cmp(&right.ingress_state())
            .then_with(|| left.egress_state.cmp(&right.egress_state))
            .then_with(|| left.steps().cmp(right.steps()))
    });
    Ok(journeys)
}

fn enumerate_machine_journeys_from_state(
    ingress_state: usize,
    current_state: usize,
    outgoing: &[Vec<JourneyStep>],
    state_stack: &mut BTreeSet<usize>,
    steps: &mut Vec<JourneyStep>,
    journeys: &mut Vec<Journey>,
    cycle_detected: &mut bool,
    budget: &mut JourneyEnumerationBudget,
) {
    if budget.exceeded || *cycle_detected {
        return;
    }

    budget.expansions += 1;
    if budget.expansions > MAX_DFS_EXPANSIONS {
        budget.exceeded = true;
        return;
    }

    if !state_stack.insert(current_state) {
        *cycle_detected = true;
        return;
    }

    let next_steps = outgoing
        .get(current_state)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if next_steps.is_empty() {
        journeys.push(Journey {
            id: JourneyId {
                ingress_state,
                steps: steps.clone(),
            },
            egress_state: current_state,
        });
        if journeys.len() > MAX_EXACT_JOURNEYS {
            budget.exceeded = true;
        }
        state_stack.remove(&current_state);
        return;
    }

    for step in next_steps {
        steps.push(*step);
        enumerate_machine_journeys_from_state(
            ingress_state,
            step.to_state,
            outgoing,
            state_stack,
            steps,
            journeys,
            cycle_detected,
            budget,
        );
        steps.pop();
        if budget.exceeded || *cycle_detected {
            break;
        }
    }

    state_stack.remove(&current_state);
}

fn render_machine_journey_diagram(machine: &CodebaseMachine, journey: &Journey) -> String {
    let mut lines = Vec::new();
    push_machine_comment_lines(&mut lines, "%%", machine);
    lines.push(format!(
        "%% journey {} :: {}",
        render_machine_cluster_label(machine),
        render_journey_label(machine, journey)
    ));
    lines.push("stateDiagram-v2".to_string());

    let mut state_indices = BTreeSet::from([journey.ingress_state(), journey.egress_state]);
    for step in journey.steps() {
        if let Some(transition) = machine.transition(step.transition) {
            state_indices.insert(transition.from);
        }
        state_indices.insert(step.to_state);
    }

    if !state_indices.is_empty() {
        lines.push(String::new());
    }
    for state_index in state_indices {
        let state = machine
            .state(state_index)
            .expect("journey state should resolve from machine");
        lines.push(format!(
            "    state \"{}\" as {}",
            escape_mermaid_label(&render_state_label(state)),
            machine.node_id(state.index)
        ));
    }

    lines.push(String::new());
    lines.push(format!(
        "    [*] --> {}",
        machine.node_id(journey.ingress_state())
    ));

    if !journey.steps().is_empty() {
        lines.push(String::new());
    }
    for (step_index, step) in journey.steps().iter().enumerate() {
        let transition = machine
            .transition(step.transition)
            .expect("journey transition should resolve from machine");
        lines.push(format!(
            "    {} --> {} : {}",
            machine.node_id(transition.from),
            machine.node_id(step.to_state),
            escape_mermaid_edge_label(&format!(
                "{}. {}",
                step_index + 1,
                transition.display_label()
            ))
        ));
    }

    lines.push(String::new());
    lines.push(format!(
        "    {} --> [*]",
        machine.node_id(journey.egress_state)
    ));

    lines.join("\n")
}

fn cross_machine_relation_groups(
    doc: &CodebaseDoc,
) -> Vec<crate::codebase::CodebaseMachineRelationGroup> {
    doc.machine_relation_groups()
        .iter()
        .filter(|group| group.from_machine != group.to_machine)
        .cloned()
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CodebaseLinkGroup {
    from_machine: usize,
    to_machine: usize,
    count: usize,
    field_names: Vec<&'static str>,
}

impl CodebaseLinkGroup {
    fn display_label(&self) -> String {
        match self.field_names.as_slice() {
            [] => format!("{} link{}", self.count, plural_suffix(self.count)),
            [name] if self.count == 1 => format!("link {name}"),
            names if names.len() <= 2 => format!("links {}", names.join(" / ")),
            _ => format!("{} link{}", self.count, plural_suffix(self.count)),
        }
    }
}

fn cross_machine_link_groups(doc: &CodebaseDoc) -> Vec<CodebaseLinkGroup> {
    let mut grouped = BTreeMap::<(usize, usize), (usize, BTreeSet<&'static str>)>::new();
    for link in doc
        .links()
        .iter()
        .filter(|link| link.from_machine != link.to_machine)
    {
        let entry = grouped
            .entry((link.from_machine, link.to_machine))
            .or_insert_with(|| (0, BTreeSet::new()));
        entry.0 += 1;
        entry.1.insert(link.display_label());
    }

    grouped
        .into_iter()
        .map(
            |((from_machine, to_machine), (count, field_names))| CodebaseLinkGroup {
                from_machine,
                to_machine,
                count,
                field_names: field_names.into_iter().collect(),
            },
        )
        .collect()
}

fn selected_machine_indices(
    doc: &CodebaseDoc,
    machine_indices: Option<&[usize]>,
) -> Result<Vec<usize>, DiagramError> {
    let Some(machine_indices) = machine_indices else {
        return Ok(doc.machines().iter().map(|machine| machine.index).collect());
    };

    let mut selected = BTreeSet::new();
    for &machine_index in machine_indices {
        doc.machine(machine_index)
            .ok_or(DiagramError::MissingMachine {
                index: machine_index,
            })?;
        selected.insert(machine_index);
    }
    Ok(selected.into_iter().collect())
}

fn render_machine_state_diagram(machine: &CodebaseMachine) -> String {
    let mut lines = Vec::new();
    push_machine_comment_lines(&mut lines, "%%", machine);
    lines.push("stateDiagram-v2".to_string());

    if !machine.states.is_empty() {
        lines.push(String::new());
    }

    for state in &machine.states {
        lines.push(format!(
            "    state \"{}\" as {}",
            escape_mermaid_label(&render_state_label(state)),
            machine.node_id(state.index)
        ));
    }

    let roots = machine
        .states
        .iter()
        .filter(|state| state.is_graph_root)
        .collect::<Vec<_>>();
    if !roots.is_empty() {
        lines.push(String::new());
        for state in roots {
            lines.push(format!("    [*] --> {}", machine.node_id(state.index)));
        }
    }

    if !machine.transitions.is_empty() {
        lines.push(String::new());
    }

    let mut has_outgoing = vec![false; machine.states.len()];
    for transition in &machine.transitions {
        has_outgoing[transition.from] = true;
        let from = machine.node_id(transition.from);
        for target in &transition.to {
            lines.push(format!(
                "    {from} --> {} : {}",
                machine.node_id(*target),
                escape_mermaid_edge_label(transition.display_label())
            ));
        }
    }

    let sinks = machine
        .states
        .iter()
        .filter(|state| !has_outgoing[state.index])
        .collect::<Vec<_>>();
    if !sinks.is_empty() {
        lines.push(String::new());
        for state in sinks {
            lines.push(format!("    {} --> [*]", machine.node_id(state.index)));
        }
    }

    lines.join("\n")
}

fn render_journey_label(machine: &CodebaseMachine, journey: &Journey) -> String {
    let ingress = machine
        .state(journey.ingress_state())
        .map(render_state_label)
        .unwrap_or_else(|| format!("state {}", journey.ingress_state()));
    let egress = machine
        .state(journey.egress_state)
        .map(render_state_label)
        .unwrap_or_else(|| format!("state {}", journey.egress_state));
    if journey.steps().is_empty() || journey.ingress_state() == journey.egress_state {
        ingress
    } else {
        format!("{ingress} -> {egress}")
    }
}

fn render_relation_sequence(detail: CodebaseRelationDetail<'_>) -> String {
    let mut lines = vec![
        format!(
            "%% relation {} [{} / {}]",
            detail.relation.index,
            detail.relation.semantic.display_label(),
            detail.relation.basis.display_label()
        ),
        "sequenceDiagram".to_string(),
    ];

    let participants = sequence_participant_machines(&detail);
    for machine in participants {
        lines.push(format!(
            "    participant {} as {}",
            sequence_participant_id(machine.index),
            escape_sequence_label(&render_machine_cluster_label(machine))
        ));
    }

    lines.push(String::new());

    let source_id = sequence_participant_id(detail.source_machine.index);
    let target_id = sequence_participant_id(detail.target_machine.index);

    match detail.relation.source {
        CodebaseRelationSource::TransitionParam { .. } => {
            let consumer_transition = detail
                .source_transition
                .expect("transition-param relations should resolve source transitions");
            let consumer_label = escape_sequence_label(consumer_transition.display_label());
            let target_state_label =
                escape_sequence_label(&render_state_label(detail.target_state));

            if detail.attested_via_producers.is_empty() {
                lines.push(format!(
                    "    {target_id}->>{source_id}: {target_state_label} for {consumer_label}"
                ));
            } else if detail.attested_via_producers.len() == 1 {
                let producer = &detail.attested_via_producers[0];
                let producer_id = sequence_participant_id(producer.machine.index);
                lines.push(format!(
                    "    Note over {producer_id}: producer {} from {} to {}",
                    escape_sequence_label(producer.transition.display_label()),
                    escape_sequence_label(&render_state_label(producer.state)),
                    target_state_label
                ));
                lines.push(format!(
                    "    {producer_id}->>{source_id}: {} via {}",
                    target_state_label,
                    escape_sequence_label(
                        detail
                            .relation
                            .attested_via
                            .as_ref()
                            .expect("attested relation should resolve route")
                            .route_name
                    )
                ));
                lines.push(format!(
                    "    Note over {source_id}: consumer {}",
                    consumer_label
                ));
            } else {
                let route_name = escape_sequence_label(
                    detail
                        .relation
                        .attested_via
                        .as_ref()
                        .expect("attested relation should resolve route")
                        .route_name,
                );
                for (index, producer) in detail.attested_via_producers.iter().enumerate() {
                    let keyword = if index == 0 { "alt" } else { "else" };
                    let producer_id = sequence_participant_id(producer.machine.index);
                    lines.push(format!(
                        "    {keyword} {} from {}",
                        escape_sequence_label(producer.transition.display_label()),
                        escape_sequence_label(&render_state_label(producer.state))
                    ));
                    lines.push(format!(
                        "        {producer_id}->>{source_id}: {} via {}",
                        target_state_label, route_name
                    ));
                }
                lines.push("    end".to_string());
                lines.push(format!(
                    "    Note over {source_id}: consumer {}",
                    consumer_label
                ));
            }
        }
        CodebaseRelationSource::StatePayload { .. } => lines.push(format!(
            "    {}: state payload carries {} at {}",
            sequence_note_over(&source_id, &target_id),
            escape_sequence_label(&render_machine_cluster_label(detail.target_machine)),
            escape_sequence_label(&render_state_label(detail.target_state))
        )),
        CodebaseRelationSource::MachineField { .. } => lines.push(format!(
            "    {}: machine field carries {} at {}",
            sequence_note_over(&source_id, &target_id),
            escape_sequence_label(&render_machine_cluster_label(detail.target_machine)),
            escape_sequence_label(&render_state_label(detail.target_state))
        )),
    }

    lines.join("\n")
}

fn render_state_label(state: &CodebaseState) -> String {
    let base = state.display_label();
    if state.direct_construction_available {
        format!("{base} [build]")
    } else {
        base.into_owned()
    }
}

fn render_machine_cluster_label(machine: &CodebaseMachine) -> String {
    if machine.role.is_composition() {
        format!("{} [composition]", machine.display_label())
    } else {
        machine.display_label().into_owned()
    }
}

fn render_workspace_machine_label(machine: &CodebaseMachine, compact_labels: bool) -> String {
    let mut label = machine.display_label().into_owned();
    if compact_labels {
        if label.contains("::") {
            label = compact_machine_type_label(&label);
        } else if let Some(stripped) = label.strip_suffix(" Machine") {
            label = stripped.to_owned();
        } else if let Some(stripped) = label.strip_suffix(" machine") {
            label = stripped.to_owned();
        }
    }

    label
}

fn workspace_machine_labels(
    doc: &CodebaseDoc,
    machine_indices: &[usize],
    compact_labels: bool,
) -> BTreeMap<usize, String> {
    let mut labels = machine_indices
        .iter()
        .filter_map(|index| {
            let machine = doc.machine(*index)?;
            Some((
                *index,
                render_workspace_machine_label(machine, compact_labels),
            ))
        })
        .collect::<BTreeMap<_, _>>();

    if !compact_labels {
        return labels;
    }

    for duplicate_indices in duplicate_workspace_labels(&labels).values() {
        for machine_index in duplicate_indices {
            if let Some(machine) = doc.machine(*machine_index) {
                labels.insert(*machine_index, render_workspace_machine_full_label(machine));
            }
        }
    }

    for duplicate_indices in duplicate_workspace_labels(&labels).values() {
        for machine_index in duplicate_indices {
            if let Some(machine) = doc.machine(*machine_index) {
                labels.insert(
                    *machine_index,
                    render_workspace_machine_exact_label(machine),
                );
            }
        }
    }

    labels
}

fn duplicate_workspace_labels(labels: &BTreeMap<usize, String>) -> BTreeMap<String, Vec<usize>> {
    let mut grouped = BTreeMap::<String, Vec<usize>>::new();
    for (machine_index, label) in labels {
        grouped
            .entry(label.clone())
            .or_default()
            .push(*machine_index);
    }
    grouped.retain(|_, machine_indices| machine_indices.len() > 1);
    grouped
}

fn render_workspace_machine_full_label(machine: &CodebaseMachine) -> String {
    machine.display_label().into_owned()
}

fn render_workspace_machine_exact_label(machine: &CodebaseMachine) -> String {
    machine.rust_type_path.to_owned()
}

/// Compacts one Rust machine type path into a shorter user-facing label.
pub fn compact_machine_type_label(label: &str) -> String {
    let segments = label.split("::").collect::<Vec<_>>();
    if segments.len() < 2 {
        return label.to_owned();
    }

    let type_name = *segments.last().expect("type path should have one segment");
    let module_name = segments
        .iter()
        .rev()
        .skip(1)
        .copied()
        .find(|segment| {
            !matches!(
                *segment,
                "machine" | "machines" | "flow" | "flows" | "protocol" | "protocols"
            )
        })
        .unwrap_or(segments[segments.len() - 2]);

    match type_name {
        "Machine" | "Flow" => format!("{module_name}::{type_name}<State>"),
        _ if module_name == type_name => type_name.to_owned(),
        _ => format!("{module_name}::{type_name}"),
    }
}

fn render_workspace_relation_group_label(
    group: &crate::codebase::CodebaseMachineRelationGroup,
    label_mode: WorkspaceFlowEdgeLabelMode,
) -> String {
    match label_mode {
        WorkspaceFlowEdgeLabelMode::Full => group.display_label(),
        WorkspaceFlowEdgeLabelMode::Compact => {
            let semantic = match group.semantic {
                crate::codebase::CodebaseMachineRelationGroupSemantic::Exact => "handoff",
                crate::codebase::CodebaseMachineRelationGroupSemantic::CompositionDirectChild => {
                    "owns"
                }
                crate::codebase::CodebaseMachineRelationGroupSemantic::Mixed => "owns + handoff",
            };
            let count = group.relation_indices.len();
            if count <= 1 {
                semantic.to_owned()
            } else {
                format!("{semantic} x{count}")
            }
        }
        WorkspaceFlowEdgeLabelMode::Hidden => String::new(),
    }
}

fn render_workspace_link_group_label(
    group: &CodebaseLinkGroup,
    label_mode: WorkspaceFlowEdgeLabelMode,
) -> String {
    match label_mode {
        WorkspaceFlowEdgeLabelMode::Full => group.display_label(),
        WorkspaceFlowEdgeLabelMode::Compact => {
            if group.count <= 1 {
                "ref".to_owned()
            } else {
                format!("ref x{}", group.count)
            }
        }
        WorkspaceFlowEdgeLabelMode::Hidden => String::new(),
    }
}

fn render_workspace_machine_node(
    machine: &CodebaseMachine,
    label: &str,
    role_shapes: bool,
) -> String {
    let escaped = escape_mermaid_label(label);
    if !role_shapes {
        return format!("    {}[\"{}\"]", machine.cluster_id(), escaped);
    }

    if machine.role.is_composition() {
        format!("    {}[[\"{}\"]]", machine.cluster_id(), escaped)
    } else {
        format!("    {}[\"{}\"]", machine.cluster_id(), escaped)
    }
}

fn workspace_relation_arrow(
    semantic: crate::codebase::CodebaseMachineRelationGroupSemantic,
) -> &'static str {
    match semantic {
        crate::codebase::CodebaseMachineRelationGroupSemantic::Exact => "-->",
        crate::codebase::CodebaseMachineRelationGroupSemantic::CompositionDirectChild
        | crate::codebase::CodebaseMachineRelationGroupSemantic::Mixed => "==>",
    }
}

fn render_workspace_flow_edge(from: String, arrow: &str, label: &str, to: String) -> String {
    if label.is_empty() {
        format!("    {from} {arrow} {to}")
    } else {
        format!(
            "    {from} {arrow}|{}| {to}",
            escape_mermaid_edge_label(label)
        )
    }
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

#[cfg(test)]
mod tests {
    use super::compact_machine_type_label;

    #[test]
    fn compact_machine_type_label_keeps_context_without_full_path_noise() {
        assert_eq!(
            compact_machine_type_label("flows::outbound_release::machine::Flow"),
            "outbound_release::Flow<State>"
        );
        assert_eq!(
            compact_machine_type_label("protocols::review::machine::Machine"),
            "review::Machine<State>"
        );
        assert_eq!(
            compact_machine_type_label("flows::audit::machine::ReviewMachine"),
            "audit::ReviewMachine"
        );
    }
}

fn sequence_participant_machines<'a>(
    detail: &'a CodebaseRelationDetail<'a>,
) -> Vec<&'a CodebaseMachine> {
    let mut participants = Vec::new();
    let mut seen = std::collections::BTreeSet::new();

    for machine in std::iter::once(detail.target_machine)
        .chain(
            detail
                .attested_via_producers
                .iter()
                .map(|producer| producer.machine),
        )
        .chain(std::iter::once(detail.source_machine))
    {
        if seen.insert(machine.index) {
            participants.push(machine);
        }
    }

    participants
}

fn sequence_participant_id(machine_index: usize) -> String {
    format!("m{machine_index}")
}

fn sequence_note_over(source_id: &str, target_id: &str) -> String {
    if source_id == target_id {
        format!("Note over {source_id}")
    } else {
        format!("Note over {source_id},{target_id}")
    }
}

fn push_machine_comment_lines(lines: &mut Vec<String>, prefix: &str, machine: &CodebaseMachine) {
    lines.push(format!(
        "{prefix} {}",
        render_machine_cluster_label(machine)
    ));

    if let Some(description) = machine.description {
        for line in description.lines() {
            lines.push(format!("{prefix} {line}"));
        }
    }

    if let Some(docs) = machine.docs {
        for line in docs.lines() {
            lines.push(format!("{prefix} {line}"));
        }
    }
}

fn escape_mermaid_label(label: &str) -> String {
    label
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn escape_mermaid_edge_label(label: &str) -> String {
    label
        .replace('&', "&amp;")
        .replace('|', "&#124;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
        .replace('\n', "<br/>")
}

fn escape_sequence_label(label: &str) -> String {
    label.replace('\n', " ").replace('"', "'")
}

fn escape_dot_label(label: &str) -> String {
    label
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn escape_plantuml_label(label: &str) -> String {
    label
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
