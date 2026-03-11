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

pub(super) fn validate_validator_coverage(
    item: &ItemImpl,
    state_enum: &EnumInfo,
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
                "Error: validator methods do not match any variant in `",
                #state_enum_name,
                "`: ",
                #unknown_list,
                ".\n",
                "Valid validator methods for this state enum are: ",
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
                "Error: missing validator methods for `",
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
        let available = available_machine_names_in_module(module_path);
        let available_line = if available.is_empty() {
            "No `#[machine]` items were found in this module.".to_string()
        } else {
            format!(
                "Available `#[machine]` items in this module: {}.",
                available.join(", ")
            )
        };
        let message = format!(
            "Error: no `#[machine]` named `{machine_name}` was found in module `{module_path}`.\n{available_line}\nFix: point `#[validators(...)]` at the right machine type in this module."
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
        let available = available_state_names_in_module(module_path);
        let available_line = if available.is_empty() {
            "No `#[state]` enums were found in this module.".to_string()
        } else {
            format!(
                "Available `#[state]` enums in this module: {}.",
                available.join(", ")
            )
        };
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
        let message = format!(
            "Error: could not resolve the `#[state]` enum for machine `{machine_name}` in module `{module_path}`.\n{expected_line}\n{available_line}"
        );
        quote! {
            compile_error!(#message);
        }
    })
}

fn available_machine_names_in_module(module_path: &str) -> Vec<String> {
    available_names_in_module(module_path, |analysis| {
        analysis
            .structs
            .iter()
            .filter(|entry| entry.attrs.iter().any(|attr| attr == "machine"))
            .map(|entry| (entry.item.ident.to_string(), entry.line_number))
            .collect()
    })
}

fn available_state_names_in_module(module_path: &str) -> Vec<String> {
    available_names_in_module(module_path, |analysis| {
        analysis
            .enums
            .iter()
            .filter(|entry| entry.attrs.iter().any(|attr| attr == "state"))
            .map(|entry| (entry.item.ident.to_string(), entry.line_number))
            .collect()
    })
}

fn available_names_in_module<F>(module_path: &str, collect: F) -> Vec<String>
where
    F: FnOnce(&macro_registry::analysis::FileAnalysis) -> Vec<(String, usize)>,
{
    let Some((file_path, _)) = current_source_info() else {
        return Vec::new();
    };
    let Some(analysis) = get_file_analysis(&file_path) else {
        return Vec::new();
    };

    let mut names = collect(&analysis)
        .into_iter()
        .filter(|(_, line_number)| {
            module_path_for_line(&file_path, *line_number).as_deref() == Some(module_path)
        })
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}
