use proc_macro2::TokenStream;
use syn::{Item, ItemStruct};

use crate::diagnostics::{DiagnosticMessage, compact_display, item_signature};
use crate::{
    ItemTarget, StateModulePath, VariantShape, lookup_loaded_state_enum, lookup_loaded_state_enum_by_name,
};

use super::metadata::is_rust_analyzer;
use super::MachineInfo;

pub fn invalid_machine_target_error(item: &Item) -> TokenStream {
    let target = ItemTarget::from(item);
    let expected_name = target.name().unwrap_or("WorkflowMachine");
    let message = DiagnosticMessage::new("#[machine] must be applied to a struct.")
        .found(item_signature(item))
        .expected(format!("`struct {expected_name}<WorkflowState> {{ ... }}`"))
        .fix(match target.name() {
            Some(name) => format!(
                "change `{name}` from {} {} into a `#[machine]` struct whose first generic names the local `#[state]` enum.",
                target.article(),
                target.kind(),
            ),
            None => "apply `#[machine]` to a struct item instead.".to_string(),
        });
    syn::Error::new(target.span(), message.render()).to_compile_error()
}

pub fn validate_machine_struct(item: &ItemStruct, machine_info: &MachineInfo) -> Option<TokenStream> {
    let machine_name = machine_info.name.clone();

    for field in &item.fields {
        let Some(attr_name) = cfg_like_attr_name(&field.attrs) else {
            continue;
        };
        let field_name = field
            .ident
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "field".to_string());
        let message = DiagnosticMessage::new(format!(
            "`#[machine]` struct `{machine_name}` field `{field_name}` uses `#[{attr_name}]`, but Statum does not support conditionally compiled machine fields."
        ))
        .found(format!("`{}`", compact_display(field)))
        .expected(format!("an unconditional `{field_name}` field in `{machine_name}`"))
        .fix("move the cfg gate to the whole `#[machine]` item or split cfg-specific field sets into separate machines.")
        .render();
        return Some(syn::Error::new_spanned(field, message).to_compile_error());
    }

    let Some(first_generic_param) = item.generics.params.first() else {
        return Some(
            syn::Error::new_spanned(
                &item.ident,
                DiagnosticMessage::new(format!(
                    "machine `{machine_name}` is missing its `#[state]` generic."
                ))
                .found(format!("`struct {machine_name} {{ ... }}`"))
                .expected(format!("`struct {machine_name}<WorkflowState> {{ ... }}`"))
                .fix(format!(
                    "declare `{machine_name}<State>` where `State` is the local `#[state]` enum."
                ))
                .render(),
            )
            .to_compile_error(),
        );
    };

    let state_path: StateModulePath = machine_info.module_path.clone();
    let matching_state_enum = machine_info
        .state_generic_name
        .as_deref()
        .and_then(|state_name| lookup_loaded_state_enum_by_name(&state_path, state_name).ok())
        .or_else(|| lookup_loaded_state_enum(&state_path).ok());

    let first_generic_param_display = compact_display(first_generic_param);
    let generics_display = compact_display(&item.generics);
    let syn::GenericParam::Type(_) = first_generic_param else {
        return Some(
            syn::Error::new_spanned(
                first_generic_param,
                DiagnosticMessage::new(format!(
                    "machine `{machine_name}` uses `{first_generic_param_display}` as its first generic, but Statum needs a type parameter naming the `#[state]` enum."
                ))
                .found(format!("`struct {machine_name}{generics_display} {{ ... }}`"))
                .expected(format!("`struct {machine_name}<WorkflowState, ...> {{ ... }}`"))
                .fix(format!(
                    "make the first generic a type parameter naming the local `#[state]` enum, for example `{machine_name}<WorkflowState>`."
                ))
                .render(),
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
    let has_data_bearing_state = matching_state_enum
        .variants
        .iter()
        .any(|variant| !matches!(variant.shape, VariantShape::Unit));

    let missing_derives: Vec<String> = machine_derives
        .iter()
        .filter(|derive| !state_derives.contains(derive))
        .cloned()
        .collect();

    if !missing_derives.is_empty() && !is_rust_analyzer() {
        let missing_list = missing_derives.join(", ");
        let message = DiagnosticMessage::new(format!(
            "machine `{machine_name}` derives `{missing_list}`, but `#[state]` enum `{state_name}` does not."
        ))
        .found(format!(
            "`#[derive({missing_list})] struct {machine_name}{generics_display} {{ ... }}`"
        ))
        .expected(format!("`#[derive({missing_list})] enum {state_name} {{ ... }}`"))
        .fix(format!(
            "add `#[derive({missing_list})]` to `{state_name}` so the generated state markers and `{machine_name}` stay compatible."
        ))
        .render();
        return Some(syn::Error::new_spanned(&item.ident, message).to_compile_error());
    }

    if first_generic_param_display != state_name {
        let message = DiagnosticMessage::new(format!(
            "machine `{machine_name}` uses `{first_generic_param_display}` as its state generic, but the `#[state]` enum in this module is `{state_name}`."
        ))
        .found(format!("`struct {machine_name}{generics_display} {{ ... }}`"))
        .expected(format!("`struct {machine_name}<{state_name}> {{ ... }}`"))
        .fix(format!("declare `{machine_name}<{state_name}>`."))
        .render();
        return Some(syn::Error::new_spanned(first_generic_param, message).to_compile_error());
    }

    for field in &item.fields {
        let Some(field_ident) = field.ident.as_ref() else {
            continue;
        };
        let Some(conflict) =
            reserved_builder_machine_field_conflict(field_ident.to_string().as_str(), has_data_bearing_state)
        else {
            continue;
        };
        let message = DiagnosticMessage::new(format!(
            "machine `{machine_name}` field `{field_ident}` conflicts with Statum's generated builder helper {conflict}."
        ))
        .found(format!("`{field_ident}: {}`", compact_display(&field.ty)))
        .expected(format!("a machine field name other than `{field_ident}`"))
        .fix("rename that machine field before using `#[machine]`.".to_string())
        .render();
        return Some(syn::Error::new_spanned(field_ident, message).to_compile_error());
    }

    None
}

fn reserved_builder_machine_field_conflict(
    field_name: &str,
    has_data_bearing_state: bool,
) -> Option<&'static str> {
    match field_name {
        "build" => Some("`build()`"),
        "state_data" if has_data_bearing_state => Some("`state_data(...)`"),
        _ => None,
    }
}

fn cfg_like_attr_name(attrs: &[syn::Attribute]) -> Option<&'static str> {
    attrs.iter().find_map(|attr| {
        if attr.path().is_ident("cfg") {
            Some("cfg")
        } else if attr.path().is_ident("cfg_attr") {
            Some("cfg_attr")
        } else {
            None
        }
    })
}
