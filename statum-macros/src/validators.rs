use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::{Ident, ItemImpl, Type, parse_macro_input};

use crate::VariantInfo;

mod emission;
mod resolution;
mod signatures;
mod type_equivalence;

use emission::{
    BatchBuilderContext, batch_builder_implementation, generate_validator_check,
    inject_machine_fields,
};
use resolution::{
    resolve_machine_metadata, resolve_state_enum_info, validate_validator_coverage,
};
use signatures::{
    validate_validator_return_type, validate_validator_signature, validator_state_name_from_ident,
};

struct VariantSpec {
    variant_name: String,
    has_state_data: bool,
    expected_ok_type: Type,
}

pub fn parse_validators(attr: TokenStream, item: TokenStream, module_path: &str) -> TokenStream {
    let machine_ident = parse_macro_input!(attr as Ident);
    let item_impl = parse_macro_input!(item as ItemImpl);
    let struct_ident = &item_impl.self_ty;

    let machine_metadata = match resolve_machine_metadata(module_path, &machine_ident) {
        Ok(metadata) => metadata,
        Err(err) => return err.into(),
    };

    let parsed_machine = match machine_metadata.parse() {
        Ok(parsed) => parsed,
        Err(err) => return err.into(),
    };
    let parsed_fields = parsed_machine.field_idents_and_types();

    let modified_methods = match inject_machine_fields(&item_impl.items, &parsed_fields) {
        Ok(methods) => methods,
        Err(err) => return err.into(),
    };

    let state_enum_info = match resolve_state_enum_info(module_path, &machine_metadata) {
        Ok(info) => info,
        Err(err) => return err.into(),
    };

    let validator_coverage = match validate_validator_coverage(&item_impl, &state_enum_info) {
        Ok(()) => quote! {},
        Err(err) => return err.into(),
    };

    let field_names = parsed_fields
        .iter()
        .map(|(ident, _)| ident.clone())
        .collect::<Vec<_>>();
    let machine_module_ident = format_ident!("{}", crate::to_snake_case(&machine_ident.to_string()));
    let machine_state_ty = quote! { #machine_module_ident::State };

    let (validator_checks, has_async) = match collect_validator_checks(
        &item_impl,
        &machine_ident,
        &machine_state_ty,
        &field_names,
        &state_enum_info.variants,
    ) {
        Ok(result) => result,
        Err(err) => return err.into(),
    };

    let fields_with_types = parsed_fields
        .iter()
        .map(|(ident, ty)| quote! { #ident: #ty })
        .collect::<Vec<_>>();

    if item_impl.items.is_empty() {
        return quote! {
            compile_error!("Error: No validator functions found in impl block. Add at least one `is_*` method.");
        }
        .into();
    }

    let machine_vis = parsed_machine.vis.clone();

    let async_token = if has_async {
        quote! { async }
    } else {
        quote! {}
    };

    let batch_builder_impl = batch_builder_implementation(BatchBuilderContext {
        machine_ident: &machine_ident,
        machine_module_ident: &machine_module_ident,
        struct_ident,
        machine_state_ty: &machine_state_ty,
        fields_with_types: &fields_with_types,
        field_names: &field_names,
        async_token: async_token.clone(),
        machine_vis: machine_vis.clone(),
    });

    let machine_builder_impl = quote! {
        #[allow(unused_imports)]
        use #machine_module_ident::IntoMachinesExt as _;

        #[statum::bon::bon(crate = ::statum::bon)]
        impl #struct_ident {
            #[builder(start_fn = into_machine, finish_fn = build)]
            #machine_vis #async_token fn __statum_into_machine(&self #(, #fields_with_types)*) -> core::result::Result<#machine_state_ty, statum::Error> {
                #(#validator_checks)*

                Err(statum::Error::InvalidState)
            }
            #(#modified_methods)*
        }

        #batch_builder_impl
    };

    let expanded = quote! {
        #validator_coverage
        #machine_builder_impl
    };

    expanded.into()
}

fn collect_validator_checks(
    item_impl: &ItemImpl,
    machine_ident: &Ident,
    machine_state_ty: &proc_macro2::TokenStream,
    field_names: &[Ident],
    variants: &[VariantInfo],
) -> Result<(Vec<proc_macro2::TokenStream>, bool), proc_macro2::TokenStream> {
    let mut checks = Vec::new();
    let mut has_async = false;
    let (variant_specs, variant_by_name) = build_variant_lookup(variants)?;

    for item in &item_impl.items {
        let syn::ImplItem::Fn(func) = item else {
            continue;
        };

        let Some(state_name) = validator_state_name_from_ident(&func.sig.ident) else {
            continue;
        };
        validate_validator_signature(func)?;

        let Some(spec_idx) = variant_by_name.get(&state_name) else {
            continue;
        };
        let spec = &variant_specs[*spec_idx];
        validate_validator_return_type(func, &spec.expected_ok_type)?;

        if func.sig.asyncness.is_some() {
            has_async = true;
        }
        checks.push(generate_validator_check(
            machine_ident,
            machine_state_ty,
            field_names,
            &spec.variant_name,
            spec.has_state_data,
            func.sig.asyncness.is_some(),
        ));
    }

    Ok((checks, has_async))
}

fn build_variant_lookup(
    variants: &[VariantInfo],
) -> Result<(Vec<VariantSpec>, HashMap<String, usize>), proc_macro2::TokenStream> {
    let mut specs = Vec::with_capacity(variants.len());
    let mut variant_by_name = HashMap::with_capacity(variants.len() * 2);

    for variant in variants {
        let state_data_type = variant.parse_data_type()?;
        specs.push(VariantSpec {
            variant_name: variant.name.clone(),
            has_state_data: state_data_type.is_some(),
            expected_ok_type: state_data_type.unwrap_or_else(|| syn::parse_quote!(())),
        });
        let idx = specs.len() - 1;
        variant_by_name.insert(variant.name.clone(), idx);
        variant_by_name.insert(crate::to_snake_case(&variant.name), idx);
    }

    Ok((specs, variant_by_name))
}
