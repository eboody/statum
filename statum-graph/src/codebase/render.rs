use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::codebase::{CodebaseDoc, CodebaseState};
use crate::render::{bundle_output_path, validate_output_stem};

/// One built-in renderer output format for codebase documents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    Mermaid,
    Dot,
    PlantUml,
    Json,
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
            escape_mermaid_label(&machine.display_label())
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
            escape_dot_label(&machine.display_label())
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
            escape_plantuml_label(&machine.display_label()),
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

fn cross_machine_relation_groups(
    doc: &CodebaseDoc,
) -> Vec<crate::codebase::CodebaseMachineRelationGroup> {
    doc.machine_relation_groups()
        .into_iter()
        .filter(|group| group.from_machine != group.to_machine)
        .collect()
}

fn render_state_label(state: &CodebaseState) -> String {
    let base = state.display_label();
    if state.direct_construction_available {
        format!("{base} [build]")
    } else {
        base.into_owned()
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
