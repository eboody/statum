use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote};
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
    ValidatorDiagnosticContext, validate_validator_return_type, validate_validator_signature,
    validator_state_name_from_ident,
};

struct VariantSpec {
    variant_name: String,
    has_state_data: bool,
    expected_ok_type: Type,
}

struct CollectValidatorContext<'a> {
    machine_ident: &'a Ident,
    machine_state_ty: &'a proc_macro2::TokenStream,
    field_names: &'a [Ident],
    persisted_type_display: &'a str,
    machine_name: &'a str,
    state_enum_name: &'a str,
}

struct IntoMachineBuilderContext<'a> {
    builder_ident: &'a Ident,
    struct_ident: &'a Type,
    machine_state_ty: &'a proc_macro2::TokenStream,
    field_names: &'a [Ident],
    field_types: &'a [Type],
    validator_checks: &'a [proc_macro2::TokenStream],
    async_token: &'a proc_macro2::TokenStream,
    machine_vis: &'a syn::Visibility,
}

pub fn parse_validators(attr: TokenStream, item: TokenStream, module_path: &str) -> TokenStream {
    let machine_ident = parse_macro_input!(attr as Ident);
    let item_impl = parse_macro_input!(item as ItemImpl);
    let struct_ident = &item_impl.self_ty;
    let persisted_type_display = struct_ident.to_token_stream().to_string();

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

    let validator_coverage = match validate_validator_coverage(
        &item_impl,
        &state_enum_info,
        &persisted_type_display,
        &machine_ident.to_string(),
    ) {
        Ok(()) => quote! {},
        Err(err) => return err.into(),
    };

    let field_names = parsed_fields
        .iter()
        .map(|(ident, _)| ident.clone())
        .collect::<Vec<_>>();
    let field_types = parsed_fields
        .iter()
        .map(|(_, ty)| ty.clone())
        .collect::<Vec<_>>();
    let machine_module_ident = format_ident!("{}", crate::to_snake_case(&machine_ident.to_string()));
    let machine_state_ty = quote! { #machine_module_ident::State };
    let machine_name = machine_ident.to_string();

    let collect_context = CollectValidatorContext {
        machine_ident: &machine_ident,
        machine_state_ty: &machine_state_ty,
        field_names: &field_names,
        persisted_type_display: &persisted_type_display,
        machine_name: &machine_name,
        state_enum_name: &state_enum_info.name,
    };

    let (validator_checks, has_async) = match collect_validator_checks(
        &item_impl,
        &state_enum_info.variants,
        &collect_context,
    ) {
        Ok(result) => result,
        Err(err) => return err.into(),
    };

    if item_impl.items.is_empty() {
        let expected_methods = state_enum_info
            .variants
            .iter()
            .map(|variant| format!("is_{}", crate::to_snake_case(&variant.name)))
            .collect::<Vec<_>>()
            .join(", ");
        let state_enum_name = state_enum_info.name.clone();
        return quote! {
            compile_error!(concat!(
                "Error: `#[validators(",
                stringify!(#machine_ident),
                ")]` on `impl ",
                #persisted_type_display,
                "` must define at least one validator method.\n",
                "Expected one method per `",
                #state_enum_name,
                "` variant: ",
                #expected_methods,
                "."
            ));
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
        field_names: &field_names,
        field_types: &field_types,
        async_token: async_token.clone(),
        machine_vis: machine_vis.clone(),
    });

    let into_machine_builder_ident = format_ident!("__Statum{}IntoMachine", machine_ident);
    let into_machine_builder_impl = generate_into_machine_builder(IntoMachineBuilderContext {
        builder_ident: &into_machine_builder_ident,
        struct_ident,
        machine_state_ty: &machine_state_ty,
        field_names: &field_names,
        field_types: &field_types,
        validator_checks: &validator_checks,
        async_token: &async_token,
        machine_vis: &machine_vis,
    });

    let machine_builder_impl = quote! {
        #[allow(unused_imports)]
        use #machine_module_ident::IntoMachinesExt as _;

        impl #struct_ident {
            #machine_vis fn into_machine(&self) -> #into_machine_builder_ident<'_> {
                #into_machine_builder_ident {
                    __statum_item: self,
                    #(
                        #field_names: core::option::Option::None
                    ),*
                }
            }

            #(#modified_methods)*
        }

        #into_machine_builder_impl
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
    variants: &[VariantInfo],
    context: &CollectValidatorContext<'_>,
) -> Result<(Vec<proc_macro2::TokenStream>, bool), proc_macro2::TokenStream> {
    let mut checks = Vec::new();
    let mut has_async = false;
    let receiver = quote! { persisted };
    let (variant_specs, variant_by_name) = build_variant_lookup(variants)?;

    for item in &item_impl.items {
        let syn::ImplItem::Fn(func) = item else {
            continue;
        };

        let Some(state_name) = validator_state_name_from_ident(&func.sig.ident) else {
            continue;
        };
        let Some(spec_idx) = variant_by_name.get(&state_name) else {
            continue;
        };
        let spec = &variant_specs[*spec_idx];
        let diagnostic_context = ValidatorDiagnosticContext {
            persisted_type_display: context.persisted_type_display,
            machine_name: context.machine_name,
            state_enum_name: context.state_enum_name,
            variant_name: &spec.variant_name,
            machine_fields: context.field_names,
            expected_ok_type: &spec.expected_ok_type,
        };
        validate_validator_signature(func, &diagnostic_context)?;
        validate_validator_return_type(func, &spec.expected_ok_type, &diagnostic_context)?;

        if func.sig.asyncness.is_some() {
            has_async = true;
        }
        checks.push(generate_validator_check(
            context.machine_ident,
            context.machine_state_ty,
            context.field_names,
            &receiver,
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

fn generate_into_machine_builder(context: IntoMachineBuilderContext<'_>) -> proc_macro2::TokenStream {
    let builder_ident = context.builder_ident;
    let struct_ident = context.struct_ident;
    let machine_state_ty = context.machine_state_ty;
    let field_names = context.field_names;
    let field_types = context.field_types;
    let validator_checks = context.validator_checks;
    let async_token = context.async_token;
    let machine_vis = context.machine_vis;
    let slot_state_idents = (0..field_names.len())
        .map(|idx| format_ident!("__STATUM_SLOT_{}_SET", idx))
        .collect::<Vec<_>>();
    let builder_defaults = if slot_state_idents.is_empty() {
        quote! { <'__statum_row> }
    } else {
        quote! { <'__statum_row, #(const #slot_state_idents: bool = false),*> }
    };
    let builder_impl_generics = if slot_state_idents.is_empty() {
        quote! { <'__statum_row> }
    } else {
        quote! { <'__statum_row, #(const #slot_state_idents: bool),*> }
    };
    let builder_ty_generics = if slot_state_idents.is_empty() {
        quote! { <'__statum_row> }
    } else {
        quote! { <'__statum_row, #(#slot_state_idents),*> }
    };
    let complete_builder_ty_generics = if slot_state_idents.is_empty() {
        quote! { <'__statum_row> }
    } else {
        let complete = slot_state_idents.iter().map(|_| quote! { true });
        quote! { <'__statum_row, #(#complete),*> }
    };
    let complete_builder_impl_generics = quote! { <'__statum_row> };

    let struct_fields = field_names
        .iter()
        .zip(field_types.iter())
        .map(|(field_name, field_type)| {
            quote! { #field_name: core::option::Option<#field_type> }
        })
        .collect::<Vec<_>>();
    let field_bindings = field_names.iter().map(|field_name| {
        let message = format!("statum internal error: `{field_name}` was not set before build");
        quote! {
            let #field_name = self.#field_name.expect(#message);
        }
    });
    let setters = field_names
        .iter()
        .zip(field_types.iter())
        .enumerate()
        .map(|(slot_idx, (field_name, field_type))| {
            let target_generics = if slot_state_idents.is_empty() {
                quote! { <'__statum_row> }
            } else {
                let generics = slot_state_idents.iter().enumerate().map(|(idx, ident)| {
                    if idx == slot_idx {
                        quote! { true }
                    } else {
                        quote! { #ident }
                    }
                });
                quote! { <'__statum_row, #(#generics),*> }
            };
            let assignments = field_names.iter().enumerate().map(|(idx, existing_field_name)| {
                if idx == slot_idx {
                    quote! { #existing_field_name: core::option::Option::Some(value) }
                } else {
                    quote! { #existing_field_name: self.#existing_field_name }
                }
            });

            quote! {
                #machine_vis fn #field_name(self, value: #field_type) -> #builder_ident #target_generics {
                    #builder_ident {
                        __statum_item: self.__statum_item,
                        #(#assignments),*
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    quote! {
        #[doc(hidden)]
        #machine_vis struct #builder_ident #builder_defaults {
            __statum_item: &'__statum_row #struct_ident,
            #(#struct_fields),*
        }

        impl #builder_impl_generics #builder_ident #builder_ty_generics {
            #(#setters)*
        }

        impl #complete_builder_impl_generics #builder_ident #complete_builder_ty_generics {
            #machine_vis #async_token fn build(self) -> core::result::Result<#machine_state_ty, statum::Error> {
                let persisted = self.__statum_item;
                #(#field_bindings)*
                #(#validator_checks)*

                Err(statum::Error::InvalidState)
            }
        }
    }
}
