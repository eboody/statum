use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::{ExportDoc, ExportSource};

/// One built-in renderer output format.
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

    /// Renders one document into this format.
    pub fn render<D>(self, doc: &D) -> String
    where
        D: ExportSource + ?Sized,
    {
        match self {
            Self::Mermaid => mermaid(doc),
            Self::Dot => dot(doc),
            Self::PlantUml => plantuml(doc),
            Self::Json => json(doc),
        }
    }

    /// Renders one document and writes it to one filesystem path.
    ///
    /// Parent directories are created when needed.
    pub fn write_to<D, P>(self, doc: &D, path: P) -> io::Result<PathBuf>
    where
        D: ExportSource + ?Sized,
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        ensure_parent_dir(path)?;
        fs::write(path, self.render(doc))?;
        Ok(path.to_path_buf())
    }
}

/// Renders one document into every built-in format and writes the resulting
/// files into `dir` using `stem` plus the format extension.
pub fn write_all_to_dir<D, P>(doc: &D, dir: P, stem: &str) -> io::Result<Vec<PathBuf>>
where
    D: ExportSource + ?Sized,
    P: AsRef<Path>,
{
    let dir = dir.as_ref();
    fs::create_dir_all(dir)?;

    Format::ALL
        .into_iter()
        .map(|format| format.write_to(doc, dir.join(format!("{stem}.{}", format.extension()))))
        .collect()
}

/// Renders a validated machine-local topology as Mermaid flowchart text.
///
/// Output ordering is deterministic for one validated export surface. State
/// labels and edge labels are escaped for Mermaid so the returned string is
/// suitable for snapshot tests, generated docs, and CLI output.
pub fn mermaid<D>(doc: &D) -> String
where
    D: ExportSource + ?Sized,
{
    let doc = doc.export_doc();
    let doc = doc.as_ref();

    let mut lines = Vec::new();
    push_comment_lines(&mut lines, "%%", doc);
    lines.push("graph TD".to_string());

    for state in doc.states() {
        lines.push(format!(
            "    {}[\"{}\"]",
            state.node_id(),
            escape_mermaid_label(&state.display_label())
        ));
    }

    if !doc.transitions().is_empty() {
        lines.push(String::new());
    }

    for transition in doc.transitions() {
        let from = doc
            .state(transition.from)
            .expect("ExportDoc transition source should exist")
            .node_id();
        for target in &transition.to {
            let to = doc
                .state(*target)
                .expect("ExportDoc transition target should exist")
                .node_id();
            lines.push(format!(
                "    {from} -->|{}| {to}",
                escape_mermaid_edge_label(transition.display_label())
            ));
        }
    }

    lines.join("\n")
}

/// Renders a validated machine-local topology as DOT text.
pub fn dot<D>(doc: &D) -> String
where
    D: ExportSource + ?Sized,
{
    let doc = doc.export_doc();
    let doc = doc.as_ref();

    let mut lines = Vec::new();
    push_comment_lines(&mut lines, "//", doc);
    lines.push(format!(
        "digraph \"{}\" {{",
        escape_dot_label(doc.machine().rust_type_path)
    ));
    lines.push("    rankdir=TB;".to_string());

    for state in doc.states() {
        lines.push(format!(
            "    {} [label=\"{}\"]",
            state.node_id(),
            escape_dot_label(&state.display_label())
        ));
    }

    if !doc.transitions().is_empty() {
        lines.push(String::new());
    }

    for transition in doc.transitions() {
        let from = doc
            .state(transition.from)
            .expect("ExportDoc transition source should exist")
            .node_id();
        for target in &transition.to {
            let to = doc
                .state(*target)
                .expect("ExportDoc transition target should exist")
                .node_id();
            lines.push(format!(
                "    {from} -> {to} [label=\"{}\"]",
                escape_dot_label(transition.display_label())
            ));
        }
    }

    lines.push("}".to_string());
    lines.join("\n")
}

/// Renders a validated machine-local topology as PlantUML state text.
pub fn plantuml<D>(doc: &D) -> String
where
    D: ExportSource + ?Sized,
{
    let doc = doc.export_doc();
    let doc = doc.as_ref();

    let mut lines = vec!["@startuml".to_string()];
    push_comment_lines(&mut lines, "'", doc);

    for state in doc.states() {
        lines.push(format!(
            "state \"{}\" as {}",
            escape_plantuml_label(&state.display_label()),
            state.node_id()
        ));
    }

    if !doc.transitions().is_empty() {
        lines.push(String::new());
    }

    for transition in doc.transitions() {
        let from = doc
            .state(transition.from)
            .expect("ExportDoc transition source should exist")
            .node_id();
        for target in &transition.to {
            let to = doc
                .state(*target)
                .expect("ExportDoc transition target should exist")
                .node_id();
            lines.push(format!(
                "{from} --> {to} : {}",
                escape_plantuml_label(transition.display_label())
            ));
        }
    }

    lines.push("@enduml".to_string());
    lines.join("\n")
}

/// Renders a validated machine-local topology as deterministic pretty JSON.
pub fn json<D>(doc: &D) -> String
where
    D: ExportSource + ?Sized,
{
    let doc = doc.export_doc();
    serde_json::to_string_pretty(doc.as_ref()).expect("ExportDoc serialization should not fail")
}

fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }

    Ok(())
}

fn push_comment_lines(lines: &mut Vec<String>, prefix: &str, doc: &ExportDoc) {
    if let Some(label) = doc.machine().label {
        for line in label.lines() {
            lines.push(format!("{prefix} {line}"));
        }
    }

    if let Some(description) = doc.machine().description {
        for line in description.lines() {
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
