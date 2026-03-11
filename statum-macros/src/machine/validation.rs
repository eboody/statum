use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{ItemStruct, LitStr};

use super::metadata::is_rust_analyzer;
use super::MachineInfo;

pub fn validate_machine_struct(item: &ItemStruct, machine_info: &MachineInfo) -> Option<TokenStream> {
    let machine_name = machine_info.name.clone();
    let Some(first_generic_param) = item.generics.params.first() else {
        let message = format!(
            "Error: #[machine] structs must declare exactly one generic type parameter naming the #[state] enum.\n\n\
Fix: declare `{machine_name}` like `pub struct {machine_name}<State> {{ ... }}`."
        );
        let message = LitStr::new(&message, Span::call_site());
        return Some(quote! {
            compile_error!(#message);
        });
    };

    if item.generics.params.len() > 1 {
        let message = format!(
            "Error: #[machine] currently supports exactly one generic type parameter.\n\n\
Fix: remove additional generics from `{machine_name}` and keep durable context in fields.\n\n\
Expected:\n\
pub struct {machine_name}<State> {{ ... }}"
        );
        let message = LitStr::new(&message, Span::call_site());
        return Some(quote! {
            compile_error!(#message);
        });
    }

    let first_generic_param_display = first_generic_param.to_token_stream().to_string();
    let syn::GenericParam::Type(_) = first_generic_param else {
        let message = format!(
            "Error: the first generic parameter of `{machine_name}` must be a type parameter naming the #[state] enum.\n\n\
Found:\n\
pub struct {machine_name}<{first_generic_param_display}> {{ ... }}"
        );
        let message = LitStr::new(&message, Span::call_site());
        return Some(quote! {
            compile_error!(#message);
        });
    };

    let matching_state_enum = match machine_info.get_matching_state_enum() {
        Ok(enum_info) => enum_info,
        Err(err) => return Some(err),
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
            "The #[state] enum `{state_name}` is missing required derives: {missing_list}\n\
Fix: Add the missing derives to your #[state] enum.\n\
Example:\n\n\
#[state]\n\
#[derive({missing_list})]\n\
pub enum State {{ Off, On }}",
        );
        let message = LitStr::new(&message, Span::call_site());
        return Some(quote! {
            compile_error!(#message);
        });
    }

    if first_generic_param_display != state_name {
        let message = format!(
            "Error: #[machine] structs must have a generic type parameter that matches the #[state] enum.\n\n\
Fix: Change the generic type parameter of `{machine_name}` to match `{state_name}`.\n\n\
Expected:\n\
pub struct {machine_name}<{state_name}> {{ ... }}\n\n\
Found:\n\
pub struct {machine_name}<{first_generic_param_display}> {{ ... }}"
        );
        let message = LitStr::new(&message, Span::call_site());
        return Some(quote! {
            compile_error!(#message);
        });
    }

    None
}
