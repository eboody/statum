use quote::quote;
use syn::{Generics, Ident, Type};

use super::shared::{
    SlotSetterContext, field_binding_tokens, field_storage_tokens, slot_setter_impls,
    slot_state_idents, slot_storage_idents,
};
use crate::machine::{builder_generics, extra_generics, generic_argument_tokens};

pub(super) struct IntoMachineBuilderContext<'a> {
    pub(super) machine_ident: &'a Ident,
    pub(super) candidate_states: &'a [proc_macro2::TokenStream],
    pub(super) builder_ident: &'a Ident,
    pub(super) struct_ident: &'a Type,
    pub(super) machine_generics: &'a Generics,
    pub(super) machine_state_ty: &'a proc_macro2::TokenStream,
    pub(super) field_names: &'a [Ident],
    pub(super) field_types: &'a [Type],
    pub(super) validator_checks: &'a [proc_macro2::TokenStream],
    pub(super) validator_report_checks: &'a [proc_macro2::TokenStream],
    pub(super) validator_explain_checks: &'a [proc_macro2::TokenStream],
    pub(super) validator_explain_storages: &'a [proc_macro2::TokenStream],
    pub(super) validator_explain_finalizers: &'a [proc_macro2::TokenStream],
    pub(super) async_token: &'a proc_macro2::TokenStream,
    pub(super) machine_vis: &'a syn::Visibility,
}

pub(super) fn generate_into_machine_builder(
    context: IntoMachineBuilderContext<'_>,
) -> proc_macro2::TokenStream {
    let machine_ident = context.machine_ident;
    let candidate_states = context.candidate_states;
    let builder_ident = context.builder_ident;
    let struct_ident = context.struct_ident;
    let machine_generics = context.machine_generics;
    let machine_state_ty = context.machine_state_ty;
    let field_names = context.field_names;
    let field_types = context.field_types;
    let validator_checks = context.validator_checks;
    let validator_report_checks = context.validator_report_checks;
    let validator_explain_checks = context.validator_explain_checks;
    let validator_explain_storages = context.validator_explain_storages;
    let validator_explain_finalizers = context.validator_explain_finalizers;
    let validator_report_count = validator_report_checks.len();
    let async_token = context.async_token;
    let machine_vis = context.machine_vis;
    let extra_machine_generics = extra_generics(machine_generics);
    let slot_storage_idents = slot_storage_idents(field_names.len());
    let slot_state_idents = slot_state_idents(field_names.len());
    let builder_defaults =
        builder_generics(&extra_machine_generics, true, &slot_state_idents, true);
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

    let struct_fields = field_storage_tokens(&slot_storage_idents, field_types);
    let field_bindings = field_binding_tokens(field_names, &slot_storage_idents);
    let setters = slot_setter_impls(
        SlotSetterContext {
            builder_ident,
            machine_vis,
            extra_machine_generics: &extra_machine_generics,
            field_names,
            field_types,
            slot_state_idents: &slot_state_idents,
            slot_storage_idents: &slot_storage_idents,
            row_lifetime: Some(quote! { '__statum_row }),
        },
        |assignments| {
            quote! {
                #builder_ident {
                    __statum_item: self.__statum_item,
                    #(#assignments),*
                }
            }
        },
    );
    let report_methods = if cfg!(feature = "rebuild-reports") {
        quote! {
            #machine_vis #async_token fn build_report(self) -> statum::RebuildReport<#machine_state_ty> {
                let __statum_persisted = self.__statum_item;
                let mut __statum_attempts = ::std::vec::Vec::with_capacity(#validator_report_count);
                #(#field_bindings)*
                #(#validator_report_checks)*

                statum::RebuildReport::new(
                    stringify!(#machine_ident),
                    statum::__private::RebuildInput {
                        type_name: core::any::type_name::<#struct_ident>(),
                        identifier: core::option::Option::None,
                    },
                    ::std::vec![#(#candidate_states),*],
                    statum::__private::RebuildAmbiguity::NotChecked,
                    __statum_attempts,
                    Err(statum::Error::InvalidState),
                )
            }

            #machine_vis #async_token fn explain(self) -> statum::RebuildReport<#machine_state_ty> {
                let __statum_persisted = self.__statum_item;
                let mut __statum_attempts = ::std::vec::Vec::with_capacity(#validator_report_count);
                let mut __statum_matched_states = ::std::vec::Vec::new();
                #(#field_bindings)*
                #(#validator_explain_storages)*
                #(#validator_explain_checks)*

                let __statum_result = match __statum_matched_states.as_slice() {
                    #(#validator_explain_finalizers,)*
                    _ => Err(statum::Error::InvalidState),
                };
                let __statum_ambiguity = if __statum_matched_states.len() > 1 {
                    statum::__private::RebuildAmbiguity::Ambiguous {
                        matched_states: __statum_matched_states,
                    }
                } else {
                    statum::__private::RebuildAmbiguity::Unambiguous
                };

                statum::RebuildReport::new(
                    stringify!(#machine_ident),
                    statum::__private::RebuildInput {
                        type_name: core::any::type_name::<#struct_ident>(),
                        identifier: core::option::Option::None,
                    },
                    ::std::vec![#(#candidate_states),*],
                    __statum_ambiguity,
                    __statum_attempts,
                    __statum_result,
                )
            }
        }
    } else {
        quote! {}
    };

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

            #report_methods

        }
    }
}
