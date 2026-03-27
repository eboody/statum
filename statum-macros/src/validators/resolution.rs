use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use macro_registry::callsite::{current_source_info, source_info_for_span_or_callsite};
use macro_registry::query;

use crate::{
    LoadedMachineLookupFailure, MachineInfo, MachinePath, format_loaded_machine_candidates,
    lookup_loaded_machine_in_module, same_named_loaded_machines_elsewhere,
};

pub(super) fn resolve_machine_metadata(
    module_path: &str,
    machine_ident: &Ident,
) -> Result<MachineInfo, TokenStream> {
    let source_info = source_info_for_span_or_callsite(machine_ident.span());
    let module_path_key: MachinePath = module_path.into();
    let machine_name = machine_ident.to_string();
    lookup_loaded_machine_in_module(&module_path_key, &machine_name).map_err(|failure| {
        let current_line = source_info.as_ref().map(|(_, line)| *line).unwrap_or_default();
        let available = available_machine_candidates_in_module(source_info.as_ref(), module_path);
        let same_named_elsewhere =
            same_named_machine_candidates_elsewhere(source_info.as_ref(), &machine_name, module_path);
        let loaded_same_named_elsewhere =
            same_named_loaded_machines_elsewhere(&module_path_key, &machine_name);
        let suggested_machine_name = available
            .first()
            .map(|candidate| candidate.name.as_str())
            .unwrap_or(machine_name.as_str());
        let available_line = if available.is_empty() {
            "No `#[machine]` items were found in this module.".to_string()
        } else {
            format!(
                "Available `#[machine]` items in this module: {}.",
                query::format_candidates(&available)
            )
        };
        let ordering_line = available
            .iter()
            .find(|candidate| {
                candidate.name == machine_name && candidate.line_number > current_line
            })
            .map(|candidate| {
                format!(
                    "Source scan found `#[machine]` item `{machine_name}` later in this module on line {}. If that item is active for this build, move it above this `#[validators]` impl because Statum resolves these relationships in expansion order.",
                    candidate.line_number
                )
            })
            .map(|line| format!("{line}\n"))
            .unwrap_or_default();
        let elsewhere_line = same_named_elsewhere
            .as_ref()
            .map(|candidates| {
                format!(
                    "Same-named `#[machine]` items elsewhere in this file: {}.",
                    query::format_candidates(candidates)
                )
            })
            .unwrap_or_else(|| "No same-named `#[machine]` items were found in other modules of this file.".to_string());
        let loaded_elsewhere_line = if loaded_same_named_elsewhere.is_empty() {
            String::new()
        } else {
            format!(
                "\nLoaded same-named `#[machine]` items elsewhere in this crate: {}.",
                format_loaded_machine_candidates(&loaded_same_named_elsewhere)
            )
        };
        let include_line = if available.is_empty()
            && same_named_elsewhere.is_none()
            && !loaded_same_named_elsewhere.is_empty()
        {
            "\nIf this `#[validators]` impl comes from an `include!()` file, Statum does not currently resolve enclosing-module `#[machine]` items from that file. Move the impl inline or into the module source file.".to_string()
        } else {
            String::new()
        };
        let missing_attr_line =
            plain_struct_line_in_module(source_info.as_ref(), module_path, &machine_name).map(
                |line| {
                    format!(
                        "A struct named `{machine_name}` exists on line {line}, but it is not annotated with `#[machine]`."
                    )
                },
            );
        let authority_line = match failure {
            LoadedMachineLookupFailure::NotFound => {
                "Statum only resolves `#[machine]` items that have already expanded before this `#[validators]` impl.".to_string()
            }
            LoadedMachineLookupFailure::Ambiguous(candidates) => format!(
                "Loaded `#[machine]` candidates were ambiguous: {}.",
                format_loaded_machine_candidates(&candidates)
            ),
        };
        let message = format!(
            "Error: no resolved `#[machine]` named `{machine_name}` was found in module `{module_path}`.\n{authority_line}\n{ordering_line}{}\n{elsewhere_line}{loaded_elsewhere_line}{include_line}\n{available_line}\nHelp: point `#[validators(...)]` at the Statum machine type in this module and declare that `#[machine]` item before this validators impl.\nCorrect shape: `#[validators({suggested_machine_name})] impl PersistedRow {{ ... }}` where `{suggested_machine_name}` is declared with `#[machine]` in `{module_path}`.",
            missing_attr_line.unwrap_or_else(|| "No plain struct with that name was found in this module either.".to_string()),
        );
        quote! {
            compile_error!(#message);
        }
    })
}

fn source_file_from_info(source_info: Option<&(String, usize)>) -> Option<String> {
    source_info
        .map(|(file_path, _)| file_path.clone())
        .or_else(|| current_source_info().map(|(file_path, _)| file_path))
}

fn available_machine_candidates_in_module(
    source_info: Option<&(String, usize)>,
    module_path: &str,
) -> Vec<query::ItemCandidate> {
    let Some(file_path) = source_file_from_info(source_info) else {
        return Vec::new();
    };
    query::candidates_in_module(&file_path, module_path, query::ItemKind::Struct, Some("machine"))
}

fn same_named_machine_candidates_elsewhere(
    source_info: Option<&(String, usize)>,
    machine_name: &str,
    module_path: &str,
) -> Option<Vec<query::ItemCandidate>> {
    let file_path = source_file_from_info(source_info)?;
    let candidates = query::same_named_candidates_elsewhere(
        &file_path,
        module_path,
        query::ItemKind::Struct,
        machine_name,
        Some("machine"),
    );
    (!candidates.is_empty()).then_some(candidates)
}

fn plain_struct_line_in_module(
    source_info: Option<&(String, usize)>,
    module_path: &str,
    struct_name: &str,
) -> Option<usize> {
    let file_path = source_file_from_info(source_info)?;
    query::plain_item_line_in_module(
        &file_path,
        module_path,
        query::ItemKind::Struct,
        struct_name,
        Some("machine"),
    )
}
