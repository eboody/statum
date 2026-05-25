use quote::{format_ident, quote};
use syn::{Generics, Ident, ImplItem, Path, Type};

use super::batch_finalization::{
    generate_batch_finalization, BatchAsyncMode, BatchFieldSource,
    BatchFinalizationOperation, BatchFinalizationPlan,
};
use super::into_machine::{generate_into_machine_builder, IntoMachineBuilderContext};
use super::shared::{
    field_binding_tokens, field_storage_tokens, generic_usage_marker_tokens,
    machine_scoped_item_path, merged_where_clause_tokens,
    prefixed_generics_argument_tokens, prefixed_generics_declaration_tokens,
    slot_setter_impls, slot_state_idents, slot_storage_idents,
    trait_extra_generic_argument_tokens, SlotSetterContext,
};
use crate::machine::{
    builder_generics, extra_generics, extra_type_arguments_tokens, generic_argument_tokens,
    machine_type_with_state,
};

pub(crate) struct BatchBuilderContext<'a> {
    pub(crate) machine_ident: &'a Ident,
    pub(crate) machine_module_path: &'a Path,
    pub(crate) machine_generics: &'a Generics,
    pub(crate) struct_ident: &'a Type,
    pub(crate) machine_state_ty: &'a proc_macro2::TokenStream,
    pub(crate) field_names: &'a [Ident],
    pub(crate) field_types: &'a [Type],
    pub(crate) async_token: proc_macro2::TokenStream,
    pub(crate) machine_vis: syn::Visibility,
}

pub(crate) struct ValidatorBuilderSurfaceContext<'a> {
    pub(crate) machine_ident: &'a Ident,
    pub(crate) machine_path: &'a Path,
    pub(crate) machine_module_path: &'a Path,
    pub(crate) machine_generics: &'a Generics,
    pub(crate) struct_ident: &'a Type,
    pub(crate) state_enum_name: &'a str,
    pub(crate) machine_state_ty: &'a proc_macro2::TokenStream,
    pub(crate) field_names: &'a [Ident],
    pub(crate) field_types: &'a [Type],
    pub(crate) validator_checks: &'a [proc_macro2::TokenStream],
    pub(crate) validator_report_checks: &'a [proc_macro2::TokenStream],
    pub(crate) modified_methods: &'a [ImplItem],
    pub(crate) async_token: &'a proc_macro2::TokenStream,
    pub(crate) machine_vis: &'a syn::Visibility,
}

pub(crate) fn validator_builder_surface(
    context: ValidatorBuilderSurfaceContext<'_>,
) -> proc_macro2::TokenStream {
    let into_machine_builder_ident =
        format_ident!("__Statum{}IntoMachine", context.machine_ident);
    let into_machines_builder_ident =
        format_ident!("__Statum{}IntoMachines", context.machine_ident);
    let into_machine_builder_impl = generate_into_machine_builder(IntoMachineBuilderContext {
        builder_ident: &into_machine_builder_ident,
        struct_ident: context.struct_ident,
        machine_generics: context.machine_generics,
        machine_state_ty: context.machine_state_ty,
        field_names: context.field_names,
        field_types: context.field_types,
        validator_checks: context.validator_checks,
        validator_report_checks: context.validator_report_checks,
        async_token: context.async_token,
        machine_vis: context.machine_vis,
    });
    let batch_builder_impl = batch_builder_implementation(BatchBuilderContext {
        machine_ident: context.machine_ident,
        machine_module_path: context.machine_module_path,
        machine_generics: context.machine_generics,
        struct_ident: context.struct_ident,
        machine_state_ty: context.machine_state_ty,
        field_names: context.field_names,
        field_types: context.field_types,
        async_token: context.async_token.clone(),
        machine_vis: context.machine_vis.clone(),
    });
    let into_machine_extra_generics = extra_generics(context.machine_generics);
    let slot_storage_idents = slot_storage_idents(context.field_names.len());
    let (into_machine_method_generics, _, into_machine_method_where_clause) =
        into_machine_extra_generics.split_for_impl();
    let into_machine_slot_defaults = (0..context.field_names.len())
        .map(|_| quote! { false })
        .collect::<Vec<_>>();
    let into_machine_builder_ty_generics = generic_argument_tokens(
        into_machine_extra_generics.params.iter(),
        Some(quote! { '_ }),
        &into_machine_slot_defaults,
    );
    let into_machines_builder_ty_generics = generic_argument_tokens(
        into_machine_extra_generics.params.iter(),
        None,
        &into_machine_slot_defaults,
    );
    let rebuild_builder_ty_generics = generic_argument_tokens(
        into_machine_extra_generics.params.iter(),
        Some(quote! { '__statum_row }),
        &into_machine_slot_defaults,
    );
    let uninitialized_state_ident =
        format_ident!("Uninitialized{}", context.state_enum_name);
    let machine_path = context.machine_path;
    let uninitialized_state_path =
        machine_scoped_item_path(machine_path, &uninitialized_state_ident);
    let uninitialized_machine_ty = machine_type_with_state(
        quote! { #machine_path },
        context.machine_generics,
        quote! { #uninitialized_state_path },
    );
    let machine_module_path = context.machine_module_path;
    let struct_ident = context.struct_ident;
    let machine_vis = context.machine_vis;
    let modified_methods = context.modified_methods;

    quote! {
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
    }
}

pub(crate) fn batch_builder_implementation(
    context: BatchBuilderContext<'_>,
) -> proc_macro2::TokenStream {
    let builder_ident = format_ident!("__Statum{}IntoMachines", context.machine_ident);
    let by_builder_ident = format_ident!("__Statum{}IntoMachinesBy", context.machine_ident);
    let machine_module_path = context.machine_module_path;
    let machine_generics = context.machine_generics;
    let struct_ident = context.struct_ident;
    let machine_state_ty = context.machine_state_ty;
    let field_names = context.field_names;
    let field_types = context.field_types;
    let async_token = context.async_token;
    let machine_vis = context.machine_vis;
    let extra_machine_generics = extra_generics(machine_generics);
    let extra_machine_ty_args = extra_type_arguments_tokens(machine_generics);
    let fields_ty = quote! { #machine_module_path::Fields #extra_machine_ty_args };
    let extra_impl_params = extra_machine_generics
        .params
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let extra_trait_args = trait_extra_generic_argument_tokens(&extra_machine_generics);
    let into_machines_impl_generics = if extra_impl_params.is_empty() {
        quote! { <T> }
    } else {
        quote! { <T, #(#extra_impl_params),*> }
    };
    let into_machines_where_clause = merged_where_clause_tokens(
        extra_machine_generics.where_clause.as_ref(),
        vec![quote! { T: Into<Vec<#struct_ident>> }],
    );

    let field_builder_chain = quote! { #(.#field_names(#field_names.clone()))* };
    let per_item_builder_chain = quote! { #(.#field_names(__statum_fields.#field_names))* };
    let await_token = if async_token.is_empty() {
        quote! {}
    } else {
        quote! { .await }
    };

    let implementation = generate_batch_finalization(BatchFinalizationPlan {
        operation: BatchFinalizationOperation::Build,
        async_mode: if async_token.is_empty() {
            BatchAsyncMode::Sync
        } else {
            BatchAsyncMode::Async
        },
        field_source: BatchFieldSource::SharedAcrossItems {
            field_builder_chain: &field_builder_chain,
        },
    });
    let report_implementation = generate_batch_finalization(BatchFinalizationPlan {
        operation: BatchFinalizationOperation::BuildReport,
        async_mode: if async_token.is_empty() {
            BatchAsyncMode::Sync
        } else {
            BatchAsyncMode::Async
        },
        field_source: BatchFieldSource::SharedAcrossItems {
            field_builder_chain: &field_builder_chain,
        },
    });
    let per_item_implementation = generate_batch_finalization(BatchFinalizationPlan {
        operation: BatchFinalizationOperation::Build,
        async_mode: if async_token.is_empty() {
            BatchAsyncMode::Sync
        } else {
            BatchAsyncMode::Async
        },
        field_source: BatchFieldSource::PerItemByFn {
            field_builder_chain: &per_item_builder_chain,
        },
    });
    let per_item_report_implementation = generate_batch_finalization(BatchFinalizationPlan {
        operation: BatchFinalizationOperation::BuildReport,
        async_mode: if async_token.is_empty() {
            BatchAsyncMode::Sync
        } else {
            BatchAsyncMode::Async
        },
        field_source: BatchFieldSource::PerItemByFn {
            field_builder_chain: &per_item_builder_chain,
        },
    });
    let slot_state_idents = slot_state_idents(field_names.len());
    let slot_storage_idents = slot_storage_idents(field_names.len());
    let builder_defaults =
        builder_generics(&extra_machine_generics, false, &slot_state_idents, true);
    let initial_builder_slots = slot_state_idents
        .iter()
        .map(|_| quote! { false })
        .collect::<Vec<_>>();
    let initial_builder_ty_generics = generic_argument_tokens(
        extra_machine_generics.params.iter(),
        None,
        &initial_builder_slots,
    );
    let complete_builder_slots = slot_state_idents
        .iter()
        .map(|_| quote! { true })
        .collect::<Vec<_>>();
    let complete_builder_ty_generics = generic_argument_tokens(
        extra_machine_generics.params.iter(),
        None,
        &complete_builder_slots,
    );
    let complete_builder_impl_generics_decl =
        builder_generics(&extra_machine_generics, false, &[], false);
    let (complete_builder_impl_generics, _, complete_builder_where_clause) =
        complete_builder_impl_generics_decl.split_for_impl();
    let shared_builder_where_clause = merged_where_clause_tokens(
        complete_builder_where_clause,
        field_types
            .iter()
            .map(|field_type| quote! { #field_type: core::clone::Clone })
            .collect(),
    );
    let by_builder_decl_generics =
        prefixed_generics_declaration_tokens("F", &extra_machine_generics);
    let by_builder_ty_generics =
        prefixed_generics_argument_tokens(quote! { F }, extra_machine_generics.params.iter());
    let by_builder_where_clause = merged_where_clause_tokens(
        extra_machine_generics.where_clause.as_ref(),
        vec![quote! { F: Fn(&#struct_ident) -> #fields_ty }],
    );
    let by_builder_marker_field = if extra_machine_generics.params.is_empty() {
        quote! {}
    } else {
        let marker_ty = generic_usage_marker_tokens(&extra_machine_generics);
        quote! {
            __statum_marker: core::marker::PhantomData<fn() -> #marker_ty>,
        }
    };
    let by_builder_marker_init = if extra_machine_generics.params.is_empty() {
        quote! {}
    } else {
        quote! {
            __statum_marker: core::marker::PhantomData,
        }
    };
    let field_storage = field_storage_tokens(&slot_storage_idents, field_types);
    let builder_init = slot_storage_idents.iter().map(|storage_ident| {
        quote! { #storage_ident: core::option::Option::None }
    });
    let field_bindings = field_binding_tokens(field_names, &slot_storage_idents);
    let setters = slot_setter_impls(
        SlotSetterContext {
            builder_ident: &builder_ident,
            machine_vis: &machine_vis,
            extra_machine_generics: &extra_machine_generics,
            field_names,
            field_types,
            slot_state_idents: &slot_state_idents,
            slot_storage_idents: &slot_storage_idents,
            row_lifetime: None,
        },
        |assignments| {
            quote! {
                #builder_ident {
                    __statum_items: self.__statum_items,
                    #(#assignments),*
                }
            }
        },
    );

    quote! {
        impl #into_machines_impl_generics #machine_module_path::IntoMachinesExt<#struct_ident #extra_trait_args> for T
        #into_machines_where_clause
        {
            type Builder = #builder_ident #initial_builder_ty_generics;
            type BuilderWithFields<F> = #by_builder_ident #by_builder_ty_generics;

            fn into_machines(self) -> Self::Builder {
                #builder_ident {
                    __statum_items: self.into(),
                    #(#builder_init),*
                }
            }

            fn into_machines_by<F>(self, fields: F) -> Self::BuilderWithFields<F>
            where
                F: Fn(&#struct_ident) -> #fields_ty,
            {
                #by_builder_ident {
                    __statum_items: self.into(),
                    __statum_fields_fn: fields,
                    #by_builder_marker_init
                }
            }
        }

        #[doc(hidden)]
        #machine_vis struct #builder_ident #builder_defaults {
            __statum_items: Vec<#struct_ident>,
            #(#field_storage),*
        }
        #(#setters)*

        impl #complete_builder_impl_generics #builder_ident #complete_builder_ty_generics #shared_builder_where_clause {
            #[inline(always)]
            #machine_vis #async_token fn build(self) -> Vec<core::result::Result<#machine_state_ty, statum::Error>> {
                let __statum_items = self.__statum_items;
                #(#field_bindings)*
                #implementation
            }

            #[inline(always)]
            #machine_vis #async_token fn build_reports(self) -> Vec<statum::RebuildReport<#machine_state_ty>> {
                let __statum_items = self.__statum_items;
                #(#field_bindings)*
                #report_implementation
            }
        }

        #[doc(hidden)]
        #machine_vis struct #by_builder_ident #by_builder_decl_generics {
            __statum_items: Vec<#struct_ident>,
            __statum_fields_fn: F,
            #by_builder_marker_field
        }

        impl #by_builder_decl_generics #by_builder_ident #by_builder_ty_generics
        #by_builder_where_clause
        {
            #[inline(always)]
            #machine_vis #async_token fn build(self) -> Vec<core::result::Result<#machine_state_ty, statum::Error>> {
                self.__private_finalize()#await_token
            }

            #[inline(always)]
            #machine_vis #async_token fn build_reports(self) -> Vec<statum::RebuildReport<#machine_state_ty>> {
                self.__private_finalize_reports()#await_token
            }

            #async_token fn __private_finalize(self) -> Vec<core::result::Result<#machine_state_ty, statum::Error>> {
                #per_item_implementation
            }

            #async_token fn __private_finalize_reports(self) -> Vec<statum::RebuildReport<#machine_state_ty>> {
                #per_item_report_implementation
            }
        }
    }
}
