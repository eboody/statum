use super::super::diagnostics::MissingTransitionMachineContext;
use crate::source::{SourceModuleQuery, current_source_info, format_candidates};

pub(in crate::transition) fn missing_transition_machine_context(
    machine_name: &str,
    module_path: &str,
) -> MissingTransitionMachineContext {
    let source_query = SourceModuleQuery::current(module_path);
    let current_line = current_source_info()
        .map(|(_, line)| line)
        .unwrap_or_default();
    let available = source_query.machine_candidates();
    let suggested_machine_name = available
        .first()
        .map(|candidate| candidate.name.clone())
        .unwrap_or_else(|| machine_name.to_string());
    let available_line = if available.is_empty() {
        "No `#[machine]` items were found in this module.".to_string()
    } else {
        format!(
            "Available `#[machine]` items in this module: {}.",
            format_candidates(&available)
        )
    };
    let ordering_line = available
        .iter()
        .find(|candidate| candidate.name == machine_name && candidate.line_number > current_line)
        .map(|candidate| {
            format!(
                "Source scan found `#[machine]` item `{machine_name}` later in this module on line {}. If that item is active for this build, move it above this `#[transition]` impl because Statum resolves these relationships in expansion order.",
                candidate.line_number
            )
        });
    let elsewhere_line = source_query
        .same_named_machine_candidates_elsewhere(machine_name)
        .map(|candidates| {
            format!(
                "Same-named `#[machine]` items elsewhere in this file: {}.",
                format_candidates(&candidates)
            )
        })
        .unwrap_or_else(|| {
            "No same-named `#[machine]` items were found in other modules of this file.".to_string()
        });
    let missing_attr_line = source_query.plain_machine_struct_line(machine_name).map(|line| {
        format!(
            "A struct named `{machine_name}` exists on line {line}, but it is not annotated with `#[machine]`."
        )
    });

    MissingTransitionMachineContext {
        suggested_machine_name,
        ordering_line,
        elsewhere_line,
        available_line,
        missing_attr_line,
    }
}
