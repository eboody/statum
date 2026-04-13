//! `#[validators]` subsystem: resolve target machines, validate signatures, and emit rebuild helpers.

use quote::{ToTokens, format_ident, quote};
use syn::{ItemImpl, Path, parse_macro_input};

use crate::contracts::ValidatorContract;
use crate::diagnostics::{DiagnosticMessage, compile_error_at};
use crate::machine::{
    builder_generics, extra_generics, generic_argument_tokens, machine_type_with_state,
};

mod contract;
mod emission;
mod plan;
mod resolution;
mod signatures;
mod type_equivalence;

use contract::{
    CollectValidatorContext, IntoMachineBuilderContext, build_validator_contract,
    machine_scoped_item_path, qualify_machine_field_types,
};
use emission::{
    BatchBuilderContext, batch_builder_implementation, inject_machine_fields,
};
use plan::collect_validator_checks;
use resolution::{
    resolve_machine_metadata, resolve_state_enum_info, resolve_validator_machine_attr,
    validate_validator_coverage,
};

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

    let contract = build_validator_contract(
        &machine_attr,
        machine_metadata.clone(),
        parsed_machine,
        &parsed_fields,
        state_enum_info,
        &persisted_type_display,
    );
    let ValidatorContract {
        resolved_machine,
        state_enum,
        persisted_type_display,
        machine_attr_display,
    } = contract;

    let validator_coverage = match validate_validator_coverage(
        &item_impl,
        &state_enum.enum_info,
        &persisted_type_display,
        &machine_attr_display,
        &resolved_machine.machine_name,
    ) {
        Ok(()) => quote! {},
        Err(err) => return err.into(),
    };

    let collect_context = CollectValidatorContext {
        machine_path: &resolved_machine.machine_path,
        machine_module_path: &resolved_machine.machine_module_path,
        machine_generics: resolved_machine.machine_generics(),
        field_names: &resolved_machine.field_names,
        persisted_type_display: &persisted_type_display,
        machine_name: &resolved_machine.machine_name,
        state_enum_name: &resolved_machine.state_enum_name,
    };

    let (validator_checks, validator_report_checks, has_async) = match collect_validator_checks(
        &item_impl,
        &state_enum.variants,
        &collect_context,
    ) {
        Ok(result) => result,
        Err(err) => return err.into(),
    };

    if item_impl.items.is_empty() {
        let expected_methods = state_enum
            .variants
            .iter()
            .map(|variant| format!("is_{}", crate::to_snake_case(&variant.name)))
            .collect::<Vec<_>>()
            .join(", ");
        let state_enum_name = state_enum.name.clone();
        let message = DiagnosticMessage::new(format!(
            "`#[validators({machine_attr_display})]` on `impl {persisted_type_display}` must define at least one validator method."
        ))
        .expected(format!(
            "one method per `{state_enum_name}` variant: `{expected_methods}`"
        ))
        .fix("add validator methods like `fn is_draft(&self) -> Result<(), _>`.".to_string());
        return compile_error_at(proc_macro2::Span::call_site(), &message).into();
    }

    let machine_vis = resolved_machine.parsed_machine.vis.clone();

    let async_token = if has_async {
        quote! { async }
    } else {
        quote! {}
    };

    let batch_builder_impl = batch_builder_implementation(BatchBuilderContext {
        machine_ident: &resolved_machine.machine_ident,
        machine_module_path: &resolved_machine.machine_module_path,
        machine_generics: resolved_machine.machine_generics(),
        struct_ident,
        machine_state_ty: &resolved_machine.machine_state_ty,
        field_names: &resolved_machine.field_names,
        field_types: &resolved_machine.field_types,
        async_token: async_token.clone(),
        machine_vis: machine_vis.clone(),
    });

    let into_machine_builder_ident =
        format_ident!("__Statum{}IntoMachine", resolved_machine.machine_ident);
    let into_machines_builder_ident =
        format_ident!("__Statum{}IntoMachines", resolved_machine.machine_ident);
    let into_machine_builder_impl = generate_into_machine_builder(IntoMachineBuilderContext {
        builder_ident: &into_machine_builder_ident,
        struct_ident,
        machine_generics: resolved_machine.machine_generics(),
        machine_state_ty: &resolved_machine.machine_state_ty,
        field_names: &resolved_machine.field_names,
        field_types: &resolved_machine.field_types,
        validator_checks: &validator_checks,
        validator_report_checks: &validator_report_checks,
        async_token: &async_token,
        machine_vis: &machine_vis,
    });
    let into_machine_extra_generics = extra_generics(resolved_machine.machine_generics());
    let slot_storage_idents = (0..resolved_machine.field_names.len())
        .map(|idx| format_ident!("__statum_slot_{}", idx))
        .collect::<Vec<_>>();
    let (into_machine_method_generics, _, into_machine_method_where_clause) =
        into_machine_extra_generics.split_for_impl();
    let into_machine_slot_defaults = (0..resolved_machine.field_names.len())
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
    let uninitialized_state_ident = format_ident!("Uninitialized{}", state_enum.name);
    let uninitialized_state_path =
        machine_scoped_item_path(&machine_attr.machine_path, &uninitialized_state_ident);
    let uninitialized_machine_ty = machine_type_with_state(
        quote! { #machine_path },
        resolved_machine.machine_generics(),
        quote! { #uninitialized_state_path },
    );
    let machine_module_path = &resolved_machine.machine_module_path;

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
