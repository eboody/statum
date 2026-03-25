use std::collections::HashMap;

use crate::MachineDoc;

/// Renders a machine-local topology as a Mermaid flow graph.
pub fn mermaid<S, T>(doc: &MachineDoc<S, T>) -> String
where
    S: Copy + Eq + std::hash::Hash + 'static,
    T: 'static,
{
    let state_positions: HashMap<S, usize> = doc
        .states
        .iter()
        .enumerate()
        .map(|(index, state)| (state.descriptor.id, index))
        .collect();

    let mut lines = vec!["graph TD".to_string()];
    for (index, state) in doc.states.iter().enumerate() {
        lines.push(format!(
            "    {}[\"{}\"]",
            node_id(index),
            escape_label(&state_label(
                state.descriptor.rust_name,
                state.descriptor.has_data
            ))
        ));
    }

    if !doc.edges.is_empty() {
        lines.push(String::new());
    }

    for edge in &doc.edges {
        let from = node_id(state_positions[&edge.descriptor.from]);
        for target in edge.descriptor.to {
            let to = node_id(state_positions[target]);
            lines.push(format!(
                "    {from} -->|{}| {to}",
                escape_label(edge.descriptor.method_name)
            ));
        }
    }

    lines.join("\n")
}

fn node_id(index: usize) -> String {
    format!("s{index}")
}

fn state_label(rust_name: &str, has_data: bool) -> String {
    if has_data {
        format!("{rust_name} (data)")
    } else {
        rust_name.to_string()
    }
}

fn escape_label(label: &str) -> String {
    label
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
