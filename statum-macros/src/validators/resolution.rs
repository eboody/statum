use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashSet;
use syn::{Ident, ItemImpl};

use crate::{
    EnumInfo, MachineInfo, MachinePath, StateModulePath, VariantInfo, ensure_machine_loaded_by_name,
    ensure_state_enum_loaded_by_name, get_state_enum, to_snake_case,
};

use super::signatures::validator_state_name_from_ident;

pub(super) fn has_validators(
    item: &ItemImpl,
    state_variants: &[VariantInfo],
) -> proc_macro2::TokenStream {
    if item.items.is_empty() {
        return quote! {};
    }

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

    let mut missing = Vec::new();
    for variant in state_variants {
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
        return quote! {
            compile_error!(concat!(
                "Error: missing validator methods: ",
                #missing_list,
                ".\n",
                "Fix: add one validator per state variant (snake_case), e.g. `fn is_draft(&self) -> Result<()>`."
            ));
        };
    }

    quote! {}
}

pub(super) fn resolve_machine_metadata(
    module_path: &str,
    machine_ident: &Ident,
) -> Result<MachineInfo, TokenStream> {
    let module_path_key: MachinePath = module_path.into();
    let machine_name = machine_ident.to_string();
    ensure_machine_loaded_by_name(&module_path_key, &machine_name).ok_or_else(|| {
        quote! {
            compile_error!("Error: No matching `#[machine]` found in scope. Ensure `#[validators(Machine)]` references a machine in the same module.");
        }
    })
}

pub(super) fn resolve_state_enum_info(
    module_path: &str,
    machine_metadata: &MachineInfo,
) -> Result<EnumInfo, TokenStream> {
    let state_path_key: StateModulePath = module_path.into();
    let expected_state_name = machine_metadata.expected_state_name();
    let _ = if let Some(expected_name) = expected_state_name.as_ref() {
        ensure_state_enum_loaded_by_name(&state_path_key, expected_name)
    } else {
        None
    };

    let state_enum_info = match expected_state_name {
        Some(expected_name) => ensure_state_enum_loaded_by_name(&state_path_key, &expected_name),
        None => get_state_enum(&state_path_key),
    };
    state_enum_info.ok_or_else(|| {
        quote! {
            compile_error!(
                "Error: No matching #[state] enum found in this module. \
Ensure the enum is in the same module as the machine and validators, and that the machine's first generic parameter matches the #[state] enum name."
            );
        }
    })
}
