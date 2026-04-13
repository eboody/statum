use quote::{ToTokens, format_ident, quote};
use std::collections::HashMap;
use syn::{Generics, Ident, ItemImpl, Path, Type, parse_macro_input};

use crate::VariantInfo;
use crate::machine::{
    builder_generics, extra_generics, extra_type_arguments_tokens, generic_argument_tokens,
    machine_type_with_state,
};

mod emission;
mod resolution;
mod signatures;
mod type_equivalence;

use emission::{
    BatchBuilderContext, ValidatorCheckContext, batch_builder_implementation,
    generate_validator_check, generate_validator_report_check, inject_machine_fields,
};
use resolution::{
    resolve_machine_metadata, resolve_state_enum_info, resolve_validator_machine_attr,
    validate_validator_coverage,
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
    machine_path: &'a Path,
    machine_module_path: &'a Path,
    machine_generics: &'a Generics,
    field_names: &'a [Ident],
    persisted_type_display: &'a str,
    machine_name: &'a str,
    state_enum_name: &'a str,
}

struct IntoMachineBuilderContext<'a> {
    builder_ident: &'a Ident,
    struct_ident: &'a Type,
    machine_generics: &'a Generics,
    machine_state_ty: &'a proc_macro2::TokenStream,
    field_names: &'a [Ident],
    field_types: &'a [Type],
    validator_checks: &'a [proc_macro2::TokenStream],
    validator_report_checks: &'a [proc_macro2::TokenStream],
    async_token: &'a proc_macro2::TokenStream,
    machine_vis: &'a syn::Visibility,
}

pub fn parse_validators(
    attr: proc_macro::TokenStream,
    item_impl: ItemImpl,
    module_path: &str,
) -> proc_macro::TokenStream {
    let machine_path = parse_macro_input!(attr as Path);
    let struct_ident = &item_impl.self_ty;
    let persisted_type_display = struct_ident.to_token_stream().to_string();
    let machine_attr = match resolve_validator_machine_attr(module_path, &machine_path) {
        Ok(attr) => attr,
        Err(err) => return err.into(),
    };
    let machine_ident = machine_attr.machine_ident.clone();
    let machine_name = machine_attr.machine_name.clone();
    let machine_attr_display = machine_attr.attr_display.clone();
    let machine_module_path =
        machine_support_module_path(&machine_attr.machine_path, &machine_name);

    let machine_metadata = match resolve_machine_metadata(module_path, &machine_attr) {
        Ok(metadata) => metadata,
        Err(err) => return err.into(),
    };

    let parsed_machine = match machine_metadata.parse() {
        Ok(parsed) => parsed,
        Err(err) => return err.into(),
    };
    let parsed_fields =
        qualify_machine_field_types(&parsed_machine.field_idents_and_types(), &machine_attr.machine_path);

    let validator_machine_generics = extra_generics(&parsed_machine.generics);
    let modified_methods = match inject_machine_fields(
        &item_impl.items,
        &parsed_fields,
        &validator_machine_generics,
    ) {
        Ok(methods) => methods,
        Err(err) => return err.into(),
    };

    let state_enum_info = match resolve_state_enum_info(&machine_metadata) {
        Ok(info) => info,
        Err(err) => return err.into(),
    };

    let validator_coverage = match validate_validator_coverage(
        &item_impl,
        &state_enum_info,
        &persisted_type_display,
        &machine_attr_display,
        &machine_name,
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
    let machine_extra_ty_args = extra_type_arguments_tokens(&parsed_machine.generics);
    let machine_state_ty = quote! { #machine_module_path::SomeState #machine_extra_ty_args };

    let collect_context = CollectValidatorContext {
        machine_path: &machine_attr.machine_path,
        machine_module_path: &machine_module_path,
        machine_generics: &parsed_machine.generics,
        field_names: &field_names,
        persisted_type_display: &persisted_type_display,
        machine_name: &machine_name,
        state_enum_name: &state_enum_info.name,
    };

    let (validator_checks, validator_report_checks, has_async) = match collect_validator_checks(
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
                #machine_attr_display,
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
        machine_module_path: &machine_module_path,
        machine_generics: &parsed_machine.generics,
        struct_ident,
        machine_state_ty: &machine_state_ty,
        field_names: &field_names,
        field_types: &field_types,
        async_token: async_token.clone(),
        machine_vis: machine_vis.clone(),
    });

    let into_machine_builder_ident = format_ident!("__Statum{}IntoMachine", machine_ident);
    let into_machines_builder_ident = format_ident!("__Statum{}IntoMachines", machine_ident);
    let into_machine_builder_impl = generate_into_machine_builder(IntoMachineBuilderContext {
        builder_ident: &into_machine_builder_ident,
        struct_ident,
        machine_generics: &parsed_machine.generics,
        machine_state_ty: &machine_state_ty,
        field_names: &field_names,
        field_types: &field_types,
        validator_checks: &validator_checks,
        validator_report_checks: &validator_report_checks,
        async_token: &async_token,
        machine_vis: &machine_vis,
    });
    let into_machine_extra_generics = extra_generics(&parsed_machine.generics);
    let slot_storage_idents = (0..field_names.len())
        .map(|idx| format_ident!("__statum_slot_{}", idx))
        .collect::<Vec<_>>();
    let (into_machine_method_generics, _, into_machine_method_where_clause) =
        into_machine_extra_generics.split_for_impl();
    let into_machine_slot_defaults = (0..field_names.len())
        .map(|_| quote! { false })
        .collect::<Vec<_>>();
    let into_machines_builder_ty_generics = generic_argument_tokens(
        into_machine_extra_generics.params.iter(),
        None,
        &into_machine_slot_defaults,
    );
    let into_machine_builder_ty_generics = generic_argument_tokens(
        into_machine_extra_generics.params.iter(),
        Some(quote! { '_ }),
        &into_machine_slot_defaults,
    );
    let rebuild_builder_ty_generics = generic_argument_tokens(
        into_machine_extra_generics.params.iter(),
        Some(quote! { '__statum_row }),
        &into_machine_slot_defaults,
    );
    let uninitialized_state_ident =
        format_ident!("Uninitialized{}", state_enum_info.name);
    let uninitialized_state_path =
        machine_scoped_item_path(&machine_attr.machine_path, &uninitialized_state_ident);
    let uninitialized_machine_ty = machine_type_with_state(
        quote! { #machine_path },
        &parsed_machine.generics,
        quote! { #uninitialized_state_path },
    );

    let machine_builder_impl = quote! {
        #[allow(unused_imports)]
        use #machine_module_path::IntoMachinesExt as _;

        impl #struct_ident {
            #machine_vis fn into_machine #into_machine_method_generics (&self) -> #into_machine_builder_ident #into_machine_builder_ty_generics #into_machine_method_where_clause {
                #into_machine_builder_ident {
                    __statum_item: self,
                    #(
                        #slot_storage_idents: core::option::Option::None
                    ),*
                }
            }

            #(#modified_methods)*
        }

        impl #into_machine_method_generics #uninitialized_machine_ty #into_machine_method_where_clause {
            #machine_vis fn rebuild<'__statum_row>(
                item: &'__statum_row #struct_ident,
            ) -> #into_machine_builder_ident #rebuild_builder_ty_generics {
                item.into_machine()
            }

            #machine_vis fn rebuild_many<T>(
                items: T,
            ) -> #into_machines_builder_ident #into_machines_builder_ty_generics
            where
                T: Into<Vec<#struct_ident>>,
            {
                #into_machines_builder_ident {
                    __statum_items: items.into(),
                    #(
                        #slot_storage_idents: core::option::Option::None
                    ),*
                }
            }
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
) -> Result<
    (
        Vec<proc_macro2::TokenStream>,
        Vec<proc_macro2::TokenStream>,
        bool,
    ),
    proc_macro2::TokenStream,
> {
    let mut checks = Vec::new();
    let mut report_checks = Vec::new();
    let mut has_async = false;
    let receiver = quote! { __statum_persisted };
    let (variant_specs, variant_by_name) = build_variant_lookup(variants)?;
    let emission_context = ValidatorCheckContext {
        machine_path: context.machine_path,
        machine_module_path: context.machine_module_path,
        machine_generics: context.machine_generics,
        field_names: context.field_names,
        receiver: &receiver,
    };

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
        let return_kind =
            validate_validator_return_type(func, &spec.expected_ok_type, &diagnostic_context)?;

        if func.sig.asyncness.is_some() {
            has_async = true;
        }
        checks.push(generate_validator_check(
            &emission_context,
            &spec.variant_name,
            spec.has_state_data,
            func.sig.asyncness.is_some(),
        ));
        report_checks.push(generate_validator_report_check(
            &emission_context,
            &spec.variant_name,
            spec.has_state_data,
            return_kind,
            func.sig.asyncness.is_some(),
        ));
    }

    Ok((checks, report_checks, has_async))
}

fn machine_support_module_path(machine_path: &Path, machine_name: &str) -> Path {
    let mut support_path = machine_path.clone();
    if let Some(last_segment) = support_path.segments.last_mut() {
        last_segment.ident = format_ident!("{}", crate::to_snake_case(machine_name));
    }
    support_path
}

fn machine_scoped_item_path(machine_path: &Path, item_ident: &Ident) -> Path {
    let mut scoped_path = machine_path.clone();
    if let Some(last_segment) = scoped_path.segments.last_mut() {
        last_segment.ident = item_ident.clone();
    }
    scoped_path
}

fn qualify_machine_field_types(
    parsed_fields: &[(Ident, Type)],
    machine_path: &Path,
) -> Vec<(Ident, Type)> {
    parsed_fields
        .iter()
        .map(|(ident, field_ty)| {
            (
                ident.clone(),
                qualify_machine_scoped_type(field_ty, machine_path),
            )
        })
        .collect()
}

fn qualify_machine_scoped_type(field_ty: &Type, machine_path: &Path) -> Type {
    let Type::Path(type_path) = field_ty else {
        return field_ty.clone();
    };
    if type_path.qself.is_some()
        || type_path.path.leading_colon.is_some()
        || type_path.path.segments.len() != 1
    {
        return field_ty.clone();
    }

    let Some(segment) = type_path.path.segments.last() else {
        return field_ty.clone();
    };
    let mut qualified = machine_scoped_item_path(machine_path, &segment.ident);
    if let Some(last_segment) = qualified.segments.last_mut() {
        last_segment.arguments = segment.arguments.clone();
    }

    syn::parse_quote!(#qualified)
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
    let machine_generics = context.machine_generics;
    let machine_state_ty = context.machine_state_ty;
    let field_names = context.field_names;
    let field_types = context.field_types;
    let validator_checks = context.validator_checks;
    let validator_report_checks = context.validator_report_checks;
    let validator_report_count = validator_report_checks.len();
    let async_token = context.async_token;
    let machine_vis = context.machine_vis;
    let extra_machine_generics = extra_generics(machine_generics);
    let slot_storage_idents = (0..field_names.len())
        .map(|idx| format_ident!("__statum_slot_{}", idx))
        .collect::<Vec<_>>();
    let slot_state_idents = (0..field_names.len())
        .map(|idx| format_ident!("__STATUM_SLOT_{}_SET", idx))
        .collect::<Vec<_>>();
    let builder_defaults = builder_generics(&extra_machine_generics, true, &slot_state_idents, true);
    let complete_slots = slot_state_idents
        .iter()
        .map(|_| quote! { true })
        .collect::<Vec<_>>();
    let complete_builder_ty_generics = generic_argument_tokens(
        extra_machine_generics.params.iter(),
        Some(quote! { '__statum_row }),
        &complete_slots,
    );
    let complete_builder_impl_generics_decl =
        builder_generics(&extra_machine_generics, true, &[], false);
    let (complete_builder_impl_generics, _, complete_builder_where_clause) =
        complete_builder_impl_generics_decl.split_for_impl();

    let struct_fields = field_names
        .iter()
        .zip(slot_storage_idents.iter())
        .zip(field_types.iter())
        .map(|((_, storage_ident), field_type)| {
            quote! { #storage_ident: core::option::Option<#field_type> }
        })
        .collect::<Vec<_>>();
    let field_bindings = field_names
        .iter()
        .zip(slot_storage_idents.iter())
        .map(|(field_name, storage_ident)| {
            let message = format!("statum internal error: `{field_name}` was not set before build");
            quote! {
                let #field_name = self.#storage_ident.expect(#message);
            }
        })
        .collect::<Vec<_>>();
    let setters = field_names
        .iter()
        .zip(field_types.iter())
        .enumerate()
        .map(|(slot_idx, (field_name, field_type))| {
            let available_slot_idents = slot_state_idents
                .iter()
                .enumerate()
                .filter_map(|(idx, ident)| (idx != slot_idx).then_some(ident.clone()))
                .collect::<Vec<_>>();
            let setter_impl_generics_decl =
                builder_generics(&extra_machine_generics, true, &available_slot_idents, false);
            let (setter_impl_generics, _, setter_where_clause) =
                setter_impl_generics_decl.split_for_impl();
            let current_generics = slot_state_idents
                .iter()
                .enumerate()
                .map(|(idx, ident)| {
                    if idx == slot_idx {
                        quote! { false }
                    } else {
                        quote! { #ident }
                    }
                })
                .collect::<Vec<_>>();
            let current_ty_generics = generic_argument_tokens(
                extra_machine_generics.params.iter(),
                Some(quote! { '__statum_row }),
                &current_generics,
            );
            let generics = slot_state_idents
                .iter()
                .enumerate()
                .map(|(idx, ident)| {
                    if idx == slot_idx {
                        quote! { true }
                    } else {
                        quote! { #ident }
                    }
                })
                .collect::<Vec<_>>();
            let target_generics = generic_argument_tokens(
                extra_machine_generics.params.iter(),
                Some(quote! { '__statum_row }),
                &generics,
            );
            let assignments = slot_storage_idents.iter().enumerate().map(|(idx, storage_ident)| {
                if idx == slot_idx {
                    quote! { #storage_ident: core::option::Option::Some(value) }
                } else {
                    quote! { #storage_ident: self.#storage_ident }
                }
            });

            quote! {
                impl #setter_impl_generics #builder_ident #current_ty_generics #setter_where_clause {
                    #machine_vis fn #field_name(self, value: #field_type) -> #builder_ident #target_generics {
                        #builder_ident {
                            __statum_item: self.__statum_item,
                            #(#assignments),*
                        }
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

        #(#setters)*

        impl #complete_builder_impl_generics #builder_ident #complete_builder_ty_generics #complete_builder_where_clause {
            #machine_vis #async_token fn build(self) -> core::result::Result<#machine_state_ty, statum::Error> {
                let __statum_persisted = self.__statum_item;
                #(#field_bindings)*
                #(#validator_checks)*

                Err(statum::Error::InvalidState)
            }

            #machine_vis #async_token fn build_report(self) -> statum::RebuildReport<#machine_state_ty> {
                let __statum_persisted = self.__statum_item;
                let mut __statum_attempts = ::std::vec::Vec::with_capacity(#validator_report_count);
                #(#field_bindings)*
                #(#validator_report_checks)*

                statum::RebuildReport {
                    attempts: __statum_attempts,
                    result: Err(statum::Error::InvalidState),
                }
            }
        }
    }
}
