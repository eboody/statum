use proc_macro2::{Span, TokenStream};
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{Item, ItemStruct};

use crate::{StateModulePath, ensure_state_enum_loaded};

use super::metadata::is_rust_analyzer;
use super::MachineInfo;

pub fn invalid_machine_target_error(item: &Item) -> TokenStream {
    let (kind, name, span) = item_kind_name_and_span(item);
    let article = indefinite_article(kind);
    let message = match name {
        Some(name) => format!(
            "Error: #[machine] must be applied to a struct, but `{name}` is {article} {kind}.\nFix: declare `struct {name}<State> {{ ... }}` and apply `#[machine]` to that struct."
        ),
        None => format!(
            "Error: #[machine] must be applied to a struct, but this item is {article} {kind}.\nFix: apply `#[machine]` to a struct like `struct Machine<State> {{ ... }}`."
        ),
    };
    syn::Error::new(span, message).to_compile_error()
}

pub fn validate_machine_struct(item: &ItemStruct, machine_info: &MachineInfo) -> Option<TokenStream> {
    let machine_name = machine_info.name.clone();
    let Some(first_generic_param) = item.generics.params.first() else {
        return Some(
            syn::Error::new_spanned(
                &item.ident,
                format!(
                    "Error: machine `{machine_name}` is missing its `#[state]` generic.\nFix: declare `{machine_name}<State>` where `State` is the `#[state]` enum in this module."
                ),
            )
            .to_compile_error(),
        );
    };

    let state_path: StateModulePath = machine_info.module_path.clone().into();
    let matching_state_enum = ensure_state_enum_loaded(&state_path);

    if item.generics.params.len() > 1 {
        let generics_display = item.generics.to_token_stream().to_string();
        let expected_state_name = matching_state_enum
            .as_ref()
            .map(|state| state.name.as_str())
            .unwrap_or("State");
        let first_generic_display = first_generic_param.to_token_stream().to_string();
        let extra_generics = item
            .generics
            .params
            .iter()
            .skip(1)
            .map(|param| format!("`{}`", param.to_token_stream()))
            .collect::<Vec<_>>()
            .join(", ");
        let message = format!(
            "Error: machine `{machine_name}` declares unsupported generics `{generics_display}`.\nStatum requires exactly one generic, and it must name the `#[state]` enum `{expected_state_name}`.\nFound first generic `{first_generic_display}` and additional generics {extra_generics}.\nFix: rewrite this as `struct {machine_name}<{expected_state_name}> {{ ... }}` and move other generic data into fields or payload types."
        );
        return Some(syn::Error::new_spanned(&item.generics, message).to_compile_error());
    }

    let first_generic_param_display = first_generic_param.to_token_stream().to_string();
    let syn::GenericParam::Type(_) = first_generic_param else {
        return Some(
            syn::Error::new_spanned(
                first_generic_param,
                format!(
                    "Error: machine `{machine_name}` uses `{first_generic_param_display}` as its first generic, but Statum needs a type parameter naming the `#[state]` enum.\nFix: declare `{machine_name}<State>` where `State` is your `#[state]` enum."
                ),
            )
            .to_compile_error(),
        );
    };
    let matching_state_enum = match matching_state_enum {
        Some(enum_info) => enum_info,
        None => match machine_info.get_matching_state_enum() {
            Ok(enum_info) => enum_info,
            Err(err) => return Some(err),
        },
    };

    let machine_derives = machine_info.derives.clone();
    let state_derives = matching_state_enum.derives.clone();
    let state_name = matching_state_enum.name.clone();

    let missing_derives: Vec<String> = machine_derives
        .iter()
        .filter(|derive| !state_derives.contains(derive))
        .cloned()
        .collect();

    if !missing_derives.is_empty() && !is_rust_analyzer() {
        let missing_list = missing_derives.join(", ");
        let message = format!(
            "Error: machine `{machine_name}` derives `{missing_list}`, but `#[state]` enum `{state_name}` does not.\nFix: add `#[derive({missing_list})]` to `{state_name}` so the generated state markers and `{machine_name}` stay compatible.",
        );
        return Some(syn::Error::new_spanned(&item.ident, message).to_compile_error());
    }

    if first_generic_param_display != state_name {
        let generics_display = item.generics.to_token_stream().to_string();
        let message = format!(
            "Error: machine `{machine_name}` uses `{first_generic_param_display}` as its state generic, but the `#[state]` enum in this module is `{state_name}`.\nFix: declare `{machine_name}<{state_name}>`.\nFound: `struct {machine_name}{generics_display} {{ ... }}`."
        );
        return Some(syn::Error::new_spanned(first_generic_param, message).to_compile_error());
    }

    None
}

fn item_kind_name_and_span(item: &Item) -> (&'static str, Option<String>, Span) {
    match item {
        Item::Const(item) => ("const item", Some(item.ident.to_string()), item.ident.span()),
        Item::Enum(item) => ("enum", Some(item.ident.to_string()), item.ident.span()),
        Item::ExternCrate(item) => ("extern crate item", Some(item.ident.to_string()), item.ident.span()),
        Item::Fn(item) => ("function", Some(item.sig.ident.to_string()), item.sig.ident.span()),
        Item::ForeignMod(item) => ("foreign module", None, item.span()),
        Item::Impl(item) => ("impl block", None, item.impl_token.span()),
        Item::Macro(item) => ("macro invocation", None, item.span()),
        Item::Mod(item) => ("module", Some(item.ident.to_string()), item.ident.span()),
        Item::Static(item) => ("static item", Some(item.ident.to_string()), item.ident.span()),
        Item::Struct(item) => ("struct", Some(item.ident.to_string()), item.ident.span()),
        Item::Trait(item) => ("trait", Some(item.ident.to_string()), item.ident.span()),
        Item::TraitAlias(item) => ("trait alias", Some(item.ident.to_string()), item.ident.span()),
        Item::Type(item) => ("type alias", Some(item.ident.to_string()), item.ident.span()),
        Item::Union(item) => ("union", Some(item.ident.to_string()), item.ident.span()),
        Item::Use(item) => ("use item", None, item.span()),
        _ => ("item", None, item.span()),
    }
}

fn indefinite_article(kind: &str) -> &'static str {
    match kind.chars().next() {
        Some('a' | 'e' | 'i' | 'o' | 'u') => "an",
        _ => "a",
    }
}
