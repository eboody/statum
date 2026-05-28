mod builder;
mod builders;
mod presentation;
mod support;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ItemStruct;

use crate::MachineInfo;

use presentation::{generate_machine_introspection_impls, generate_machine_state_surface};
pub(crate) use support::transition_support_module_ident;
use support::{
    extract_state_generic_ident, generate_field_type_aliases, generate_struct_definition,
    parse_generics, transition_support,
};

pub fn generate_machine_impls(machine_info: &MachineInfo, item: &ItemStruct) -> TokenStream {
    let state_enum = match machine_info.get_matching_state_enum() {
        Ok(enum_info) => enum_info,
        Err(err) => return err,
    };
    let parsed_state = match state_enum.parse() {
        Ok(parsed) => parsed,
        Err(err) => return err,
    };
    let parsed_machine = match machine_info.parse() {
        Ok(parsed) => parsed,
        Err(err) => return err,
    };
    let machine_ident = format_ident!("{}", machine_info.name);
    let generics = match parse_generics(&parsed_machine, &state_enum) {
        Ok(generics) => generics,
        Err(err) => return err,
    };
    let state_generic_ident = match extract_state_generic_ident(&generics) {
        Ok(ident) => ident,
        Err(err) => return err,
    };
    let struct_def = match generate_struct_definition(
        &parsed_machine,
        &machine_ident,
        &generics,
        &state_generic_ident,
        &state_enum.get_trait_name(),
        &transition_support_module_ident(machine_info),
    ) {
        Ok(def) => def,
        Err(err) => return err,
    };
    let builder_methods = machine_info.generate_builder_methods(&parsed_machine, &parsed_state);
    let transition_support = transition_support(machine_info, &parsed_machine, &state_enum);
    let field_type_aliases = generate_field_type_aliases(machine_info, item);
    let machine_state_surface = match generate_machine_state_surface(
        machine_info,
        &parsed_machine,
        &parsed_state,
        &machine_ident,
    ) {
        Ok(surface) => surface,
        Err(err) => return err,
    };
    let introspection_impls = generate_machine_introspection_impls(
        machine_info,
        &state_enum,
        &generics,
        &parsed_state,
        &machine_ident,
    );

    quote! {
        #transition_support
        #field_type_aliases
        #struct_def
        #builder_methods
        #machine_state_surface
        #introspection_impls
    }
}
