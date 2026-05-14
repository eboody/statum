use proc_macro2::TokenStream;

use crate::diagnostics::{DiagnosticMessage, compile_error_at};
use crate::source::{SourceModuleQuery, current_source_info, format_candidates};
use crate::{
    EnumInfo, LoadedMachineLookupFailure, LoadedStateLookupFailure, MachineInfo, MachinePath,
    StateModulePath, format_loaded_machine_candidates, format_loaded_state_candidates,
    lookup_loaded_machine_in_module, lookup_loaded_state_enum, lookup_loaded_state_enum_by_name,
    lookup_unique_loaded_machine_by_name,
};

use super::attr::{
    ValidatorMachineAttr, machine_attr_display_for_module, unresolved_relative_validator_path_line,
};

pub(crate) fn resolve_machine_metadata(
    current_module_path: &str,
    machine_attr: &ValidatorMachineAttr,
) -> Result<MachineInfo, TokenStream> {
    let module_path = machine_attr.machine_module_path.as_str();
    let source_query = SourceModuleQuery::current(module_path);
    let module_path_key: MachinePath = module_path.into();
    let machine_name = machine_attr.machine_name.as_str();
    lookup_loaded_machine_in_module(&module_path_key, machine_name).map_err(|failure| {
        let current_line = current_source_info().map(|(_, line)| line).unwrap_or_default();
        let available = source_query.machine_candidates();
        let suggested_machine_name = available
            .first()
            .map(|candidate| candidate.name.as_str())
            .unwrap_or(machine_name);
        let suggested_attr = preferred_machine_attr_suggestion(
            current_module_path,
            machine_name,
            Some(module_path),
            suggested_machine_name,
        )
        .unwrap_or_else(|| format!("crate::path::{suggested_machine_name}"));
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
                    "Source scan found `#[machine]` item `{machine_name}` later in this module on line {}. If that item is active for this build, move it above this `#[validators]` impl because Statum resolves these relationships in expansion order.",
                    candidate.line_number
                )
            })
            .map(|line| format!("{line}\n"))
            .unwrap_or_default();
        let elsewhere_line = source_query.same_named_machine_candidates_elsewhere(machine_name)
            .map(|candidates| {
                format!(
                    "Same-named `#[machine]` items elsewhere in this file: {}.",
                    format_candidates(&candidates)
                )
            })
            .unwrap_or_else(|| "No same-named `#[machine]` items were found in other modules of this file.".to_string());
        let missing_attr_line = source_query.plain_machine_struct_line(machine_name).map(|line| {
            format!(
                "A struct named `{machine_name}` exists on line {line}, but it is not annotated with `#[machine]`."
            )
        });
        let relative_path_line = unresolved_relative_validator_path_line(machine_attr, &suggested_attr)
            .map(|line| format!("{line}\n"))
            .unwrap_or_default();
        let authority_line = match failure {
            LoadedMachineLookupFailure::NotFound => {
                "Statum only resolves `#[machine]` items that have already expanded before this `#[validators]` impl.".to_string()
            }
            LoadedMachineLookupFailure::Ambiguous(candidates) => format!(
                "Loaded `#[machine]` candidates were ambiguous: {}.",
                format_loaded_machine_candidates(&candidates)
            ),
        };
        let message = DiagnosticMessage::new(format!(
            "`#[validators({})]` could not resolve a matching `#[machine]` in module `{module_path}`.",
            machine_attr.attr_display,
        ))
        .found(format!("`#[validators({})]`", machine_attr.attr_display))
        .expected(format!("`#[validators({suggested_attr})]`"))
        .fix("point `#[validators(...)]` at the Statum machine type declared in that module and declare that `#[machine]` item before this validators impl.".to_string())
        .reason(authority_line)
        .assumption(relative_path_line.trim_end().to_string())
        .note(ordering_line.trim_end().to_string())
        .note(
            missing_attr_line
                .unwrap_or_else(|| "No plain struct with that name was found in this module either.".to_string()),
        )
        .candidates(elsewhere_line)
        .candidates(available_line)
        .help(format!(
            "Correct shape: `#[validators({suggested_attr})] impl PersistedRow {{ ... }}`."
        ));
        compile_error_at(proc_macro2::Span::call_site(), &message)
    })
}

pub(crate) fn resolve_state_enum_info(machine_metadata: &MachineInfo) -> Result<EnumInfo, TokenStream> {
    let module_path = machine_metadata.module_path.as_ref();
    let source_query = SourceModuleQuery::current(module_path);
    let state_path_key: StateModulePath = module_path.into();
    let machine_name = machine_metadata.name.clone();
    let expected_state_name = machine_metadata.state_generic_name.as_deref();
    let state_enum_info = match expected_state_name {
        Some(expected_name) => lookup_loaded_state_enum_by_name(&state_path_key, expected_name),
        None => lookup_loaded_state_enum(&state_path_key),
    };
    state_enum_info.map_err(|failure| {
        let current_line = current_source_info().map(|(_, line)| line).unwrap_or_default();
        let available = source_query.state_candidates();
        let available_line = if available.is_empty() {
            "No `#[state]` enums were found in this module.".to_string()
        } else {
            format!(
                "Available `#[state]` enums in this module: {}.",
                format_candidates(&available)
            )
        };
        let elsewhere_line = expected_state_name
            .and_then(|name| source_query.same_named_state_candidates_elsewhere(name))
            .map(|candidates| {
                format!(
                    "Same-named `#[state]` enums elsewhere in this file: {}.",
                    format_candidates(&candidates)
                )
            })
            .unwrap_or_else(|| "No same-named `#[state]` enums were found in other modules of this file.".to_string());
        let expected_line = expected_state_name
            .map(|name| {
                format!(
                    "Machine `{machine_name}` expects its first generic parameter to name `#[state]` enum `{name}`."
                )
            })
            .unwrap_or_else(|| {
                format!(
                    "Machine `{machine_name}` did not expose a resolvable first generic parameter for its `#[state]` enum."
                )
            });
        let ordering_line = expected_state_name.and_then(|name| {
            available
                .iter()
                .find(|candidate| candidate.name == name && candidate.line_number > current_line)
                .map(|candidate| {
                    format!(
                        "Source scan found `#[state]` enum `{name}` later in this module on line {}. If that item is active for this build, move it above the machine and this `#[validators]` impl because Statum resolves these relationships in expansion order.",
                        candidate.line_number
                    )
                })
        });
        let ordering_line = ordering_line
            .map(|line| format!("{line}\n"))
            .unwrap_or_default();
        let missing_attr_line = expected_state_name.as_ref().and_then(|name| {
            source_query.plain_state_enum_line(name).map(|line| {
                format!("An enum named `{name}` exists on line {line}, but it is not annotated with `#[state]`.")
            })
        });
        let authority_line = match failure {
            LoadedStateLookupFailure::NotFound => {
                "Statum only resolves `#[state]` enums that have already expanded before this `#[validators]` impl.".to_string()
            }
            LoadedStateLookupFailure::Ambiguous(candidates) => format!(
                "Loaded `#[state]` candidates were ambiguous: {}.",
                format_loaded_state_candidates(&candidates)
            ),
        };
        let message = DiagnosticMessage::new(format!(
            "machine `{machine_name}` could not resolve its `#[state]` enum in module `{module_path}` for this `#[validators]` impl."
        ))
        .expected(format!(
            "`struct {machine_name}<ExpectedState> {{ ... }}` where `ExpectedState` is a `#[state]` enum declared in `{module_path}`"
        ))
        .fix("make the machine's first generic name the right local `#[state]` enum and declare that enum before the machine and validators impl.".to_string())
        .reason(expected_line)
        .note(authority_line)
        .note(ordering_line.trim_end().to_string())
        .note(
            missing_attr_line
                .unwrap_or_else(|| "No plain enum with that expected name was found in this module either.".to_string()),
        )
        .candidates(elsewhere_line)
        .candidates(available_line);
        compile_error_at(proc_macro2::Span::call_site(), &message)
    })
}

pub(crate) fn preferred_machine_attr_suggestion(
    current_module_path: &str,
    machine_name: &str,
    fallback_module_path: Option<&str>,
    fallback_machine_name: &str,
) -> Option<String> {
    if let Ok(machine_info) = lookup_unique_loaded_machine_by_name(machine_name) {
        return Some(machine_attr_display_for_module(
            current_module_path,
            machine_info.module_path.as_ref(),
            &machine_info.name,
        ));
    }

    fallback_module_path.map(|module_path| {
        machine_attr_display_for_module(current_module_path, module_path, fallback_machine_name)
    })
}
