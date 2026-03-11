use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashSet;
use syn::{Ident, ItemImpl};

use macro_registry::analysis::get_file_analysis;
use macro_registry::callsite::{current_source_info, module_path_for_line};

use crate::{
    EnumInfo, MachineInfo, MachinePath, StateModulePath, ensure_machine_loaded_by_name,
    ensure_state_enum_loaded_by_name, get_state_enum, to_snake_case,
};

use super::signatures::validator_state_name_from_ident;

#[derive(Clone)]
struct ItemCandidate {
    name: String,
    line_number: usize,
    module_path: String,
}

pub(super) fn validate_validator_coverage(
    item: &ItemImpl,
    state_enum: &EnumInfo,
    persisted_type_display: &str,
    machine_name: &str,
) -> Result<(), proc_macro2::TokenStream> {
    if item.items.is_empty() {
        return Ok(());
    }

    let valid_state_names = state_enum
        .variants
        .iter()
        .map(|variant| to_snake_case(&variant.name))
        .collect::<HashSet<_>>();
    let existing = item
        .items
        .iter()
        .filter_map(|item| {
            if let syn::ImplItem::Fn(func) = item {
                validator_state_name_from_ident(&func.sig.ident)
            } else {
                None
            }
        })
        .collect::<HashSet<_>>();
    let unknown = existing
        .iter()
        .filter(|name| !valid_state_names.contains(*name))
        .map(|name| format!("is_{name}"))
        .collect::<Vec<_>>();

    if !unknown.is_empty() {
        let unknown_list = unknown.join(", ");
        let state_enum_name = &state_enum.name;
        let valid_list = state_enum
            .variants
            .iter()
            .map(|variant| format!("is_{}", to_snake_case(&variant.name)))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(quote! {
            compile_error!(concat!(
                "Error: `#[validators(",
                #machine_name,
                ")]` on `impl ",
                #persisted_type_display,
                "` defines methods that do not match any variant in `",
                #state_enum_name,
                "`: ",
                #unknown_list,
                ".\n",
                "Valid validator methods for `",
                #machine_name,
                "` are: ",
                #valid_list,
                "."
            ));
        });
    }

    let mut missing = Vec::new();
    for variant in &state_enum.variants {
        let variant_name = to_snake_case(&variant.name);
        if !existing.contains(&variant_name) {
            missing.push(variant_name);
        }
    }

    if !missing.is_empty() {
        let missing_list = missing
            .iter()
            .map(|name| format!("is_{name}"))
            .collect::<Vec<_>>()
            .join(", ");
        let state_enum_name = &state_enum.name;
        return Err(quote! {
            compile_error!(concat!(
                "Error: `#[validators(",
                #machine_name,
                ")]` on `impl ",
                #persisted_type_display,
                "` is missing validator methods for `",
                #state_enum_name,
                "`: ",
                #missing_list,
                ".\n",
                "Fix: add one validator per state variant (snake_case), e.g. `fn is_draft(&self) -> Result<()>`."
            ));
        });
    }

    Ok(())
}

pub(super) fn resolve_machine_metadata(
    module_path: &str,
    machine_ident: &Ident,
) -> Result<MachineInfo, TokenStream> {
    let module_path_key: MachinePath = module_path.into();
    let machine_name = machine_ident.to_string();
    ensure_machine_loaded_by_name(&module_path_key, &machine_name).ok_or_else(|| {
        let available = available_machine_candidates_in_module(module_path);
        let suggested_machine_name = available
            .first()
            .map(|candidate| candidate.name.as_str())
            .unwrap_or(machine_name.as_str());
        let available_line = if available.is_empty() {
            "No `#[machine]` items were found in this module.".to_string()
        } else {
            format!(
                "Available `#[machine]` items in this module: {}.",
                format_candidates(&available)
            )
        };
        let elsewhere_line = same_named_machine_candidates_elsewhere(&machine_name, module_path)
            .map(|candidates| {
                format!(
                    "Same-named `#[machine]` items elsewhere in this file: {}.",
                    format_candidates(&candidates)
                )
            })
            .unwrap_or_else(|| "No same-named `#[machine]` items were found in other modules of this file.".to_string());
        let missing_attr_line = plain_struct_line_in_module(module_path, &machine_name).map(|line| {
            format!(
                "A struct named `{machine_name}` exists on line {line}, but it is not annotated with `#[machine]`."
            )
        });
        let message = format!(
            "Error: no `#[machine]` named `{machine_name}` was found in module `{module_path}`.\n{}\n{elsewhere_line}\n{available_line}\nHelp: point `#[validators(...)]` at the Statum machine type in this module.\nCorrect shape: `#[validators({suggested_machine_name})] impl PersistedRow {{ ... }}` where `{suggested_machine_name}` is declared with `#[machine]` in `{module_path}`.",
            missing_attr_line.unwrap_or_else(|| "No plain struct with that name was found in this module either.".to_string()),
        );
        quote! {
            compile_error!(#message);
        }
    })
}

pub(super) fn resolve_state_enum_info(
    module_path: &str,
    machine_metadata: &MachineInfo,
) -> Result<EnumInfo, TokenStream> {
    let state_path_key: StateModulePath = module_path.into();
    let machine_name = machine_metadata.name.clone();
    let expected_state_name = machine_metadata.expected_state_name();
    let _ = if let Some(expected_name) = expected_state_name.as_ref() {
        ensure_state_enum_loaded_by_name(&state_path_key, expected_name)
    } else {
        None
    };

    let state_enum_info = match expected_state_name.as_ref() {
        Some(expected_name) => ensure_state_enum_loaded_by_name(&state_path_key, expected_name),
        None => get_state_enum(&state_path_key),
    };
    state_enum_info.ok_or_else(|| {
        let available = available_state_candidates_in_module(module_path);
        let available_line = if available.is_empty() {
            "No `#[state]` enums were found in this module.".to_string()
        } else {
            format!(
                "Available `#[state]` enums in this module: {}.",
                format_candidates(&available)
            )
        };
        let elsewhere_line = expected_state_name
            .as_ref()
            .and_then(|name| same_named_state_candidates_elsewhere(name, module_path))
            .map(|candidates| {
                format!(
                    "Same-named `#[state]` enums elsewhere in this file: {}.",
                    format_candidates(&candidates)
                )
            })
            .unwrap_or_else(|| "No same-named `#[state]` enums were found in other modules of this file.".to_string());
        let expected_line = expected_state_name
            .as_ref()
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
        let missing_attr_line = expected_state_name.as_ref().and_then(|name| {
            plain_enum_line_in_module(module_path, name).map(|line| {
                format!("An enum named `{name}` exists on line {line}, but it is not annotated with `#[state]`.")
            })
        });
        let message = format!(
            "Error: could not resolve the `#[state]` enum for machine `{machine_name}` in module `{module_path}`.\n{expected_line}\n{}\n{elsewhere_line}\n{available_line}\nHelp: make sure the machine's first generic names the right `#[state]` enum in this module.\nCorrect shape: `struct {machine_name}<ExpectedState> {{ ... }}` where `ExpectedState` is a `#[state]` enum declared in `{module_path}`.",
            missing_attr_line.unwrap_or_else(|| "No plain enum with that expected name was found in this module either.".to_string())
        );
        quote! {
            compile_error!(#message);
        }
    })
}

fn available_machine_candidates_in_module(module_path: &str) -> Vec<ItemCandidate> {
    available_candidates_in_module(module_path, |analysis| {
        analysis
            .structs
            .iter()
            .filter(|entry| entry.attrs.iter().any(|attr| attr == "machine"))
            .filter_map(|entry| item_candidate_from_line(entry.item.ident.to_string(), entry.line_number))
            .collect()
    })
}

fn available_state_candidates_in_module(module_path: &str) -> Vec<ItemCandidate> {
    available_candidates_in_module(module_path, |analysis| {
        analysis
            .enums
            .iter()
            .filter(|entry| entry.attrs.iter().any(|attr| attr == "state"))
            .filter_map(|entry| item_candidate_from_line(entry.item.ident.to_string(), entry.line_number))
            .collect()
    })
}

fn available_candidates_in_module<F>(module_path: &str, collect: F) -> Vec<ItemCandidate>
where
    F: FnOnce(&macro_registry::analysis::FileAnalysis) -> Vec<ItemCandidate>,
{
    let Some((file_path, _)) = current_source_info() else {
        return Vec::new();
    };
    let Some(analysis) = get_file_analysis(&file_path) else {
        return Vec::new();
    };

    let mut names = collect(&analysis)
        .into_iter()
        .filter(|candidate| candidate.module_path == module_path)
        .collect::<Vec<_>>();
    names.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.module_path.cmp(&right.module_path))
            .then(left.line_number.cmp(&right.line_number))
    });
    names.dedup_by(|left, right| left.name == right.name && left.line_number == right.line_number);
    names
}

fn same_named_machine_candidates_elsewhere(machine_name: &str, module_path: &str) -> Option<Vec<ItemCandidate>> {
    same_named_candidates_elsewhere(module_path, |analysis| {
        analysis
            .structs
            .iter()
            .filter(|entry| entry.item.ident == machine_name && entry.attrs.iter().any(|attr| attr == "machine"))
            .filter_map(|entry| item_candidate_from_line(entry.item.ident.to_string(), entry.line_number))
            .collect()
    })
}

fn same_named_state_candidates_elsewhere(state_name: &str, module_path: &str) -> Option<Vec<ItemCandidate>> {
    same_named_candidates_elsewhere(module_path, |analysis| {
        analysis
            .enums
            .iter()
            .filter(|entry| entry.item.ident == state_name && entry.attrs.iter().any(|attr| attr == "state"))
            .filter_map(|entry| item_candidate_from_line(entry.item.ident.to_string(), entry.line_number))
            .collect()
    })
}

fn same_named_candidates_elsewhere<F>(module_path: &str, collect: F) -> Option<Vec<ItemCandidate>>
where
    F: FnOnce(&macro_registry::analysis::FileAnalysis) -> Vec<ItemCandidate>,
{
    let (file_path, _) = current_source_info()?;
    let analysis = get_file_analysis(&file_path)?;
    let mut candidates = collect(&analysis)
        .into_iter()
        .filter(|candidate| candidate.module_path != module_path)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        left.module_path
            .cmp(&right.module_path)
            .then(left.line_number.cmp(&right.line_number))
    });
    (!candidates.is_empty()).then_some(candidates)
}

fn item_candidate_from_line(name: String, line_number: usize) -> Option<ItemCandidate> {
    let (file_path, _) = current_source_info()?;
    let module_path = module_path_for_line(&file_path, line_number)?;
    Some(ItemCandidate {
        name,
        line_number,
        module_path,
    })
}

fn plain_struct_line_in_module(module_path: &str, struct_name: &str) -> Option<usize> {
    current_analysis_line_in_module(module_path, |analysis, file_path| {
        analysis.structs.iter().find_map(|entry| {
            (entry.item.ident == struct_name
                && module_path_for_line(file_path, entry.line_number).as_deref() == Some(module_path)
                && !entry.attrs.iter().any(|attr| attr == "machine"))
            .then_some(entry.line_number)
        })
    })
}

fn plain_enum_line_in_module(module_path: &str, enum_name: &str) -> Option<usize> {
    current_analysis_line_in_module(module_path, |analysis, file_path| {
        analysis.enums.iter().find_map(|entry| {
            (entry.item.ident == enum_name
                && module_path_for_line(file_path, entry.line_number).as_deref() == Some(module_path)
                && !entry.attrs.iter().any(|attr| attr == "state"))
            .then_some(entry.line_number)
        })
    })
}

fn current_analysis_line_in_module<F>(module_path: &str, find_line: F) -> Option<usize>
where
    F: FnOnce(&macro_registry::analysis::FileAnalysis, &str) -> Option<usize>,
{
    let (file_path, _) = current_source_info()?;
    let analysis = get_file_analysis(&file_path)?;
    find_line(&analysis, &file_path).filter(|_| !module_path.is_empty())
}

fn format_candidates(candidates: &[ItemCandidate]) -> String {
    candidates
        .iter()
        .map(|candidate| {
            format!(
                "`{}` in `{}` (line {})",
                candidate.name, candidate.module_path, candidate.line_number
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}
