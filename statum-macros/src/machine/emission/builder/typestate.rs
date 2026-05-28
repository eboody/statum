use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

use super::context::BuilderContext;
use crate::machine::{
    builder_generics, extra_generics, extra_type_arguments_tokens, generic_argument_tokens,
};

pub(in crate::machine::emission) fn typestate_builder_tokens(
    context: &BuilderContext<'_>,
    variant_builder_ident: &Ident,
    data_type: Option<&syn::Type>,
    machine_state_ty: &TokenStream,
    struct_initialization: TokenStream,
) -> TokenStream {
    let machine_generics = context.machine_generics;
    let builder_vis = context.builder_vis;
    let field_names = context.field_names;
    let field_types = context.field_types;
    let extra_generics = extra_generics(machine_generics);
    let extra_ty_args = extra_type_arguments_tokens(machine_generics);
    let (extra_impl_generics, _, extra_where_clause) = extra_generics.split_for_impl();
    let has_state_data = data_type.is_some();
    let slot_types = data_type
        .into_iter()
        .cloned()
        .chain(field_types.iter().cloned())
        .collect::<Vec<_>>();
    let slot_storage_idents = (0..slot_types.len())
        .map(|idx| format_ident!("__statum_slot_{}", idx))
        .collect::<Vec<_>>();
    let slot_state_idents = (0..slot_types.len())
        .map(|idx| format_ident!("__STATUM_SLOT_{}_SET", idx))
        .collect::<Vec<_>>();
    let already_set_ident = format_ident!("__Statum{}AlreadySet", variant_builder_ident);
    let struct_fields = slot_storage_idents
        .iter()
        .zip(slot_types.iter())
        .map(|(storage_ident, slot_type)| {
            quote! { #storage_ident: core::option::Option<#slot_type> }
        })
        .collect::<Vec<_>>();
    let builder_defaults = builder_generics(&extra_generics, false, &slot_state_idents, true);
    let builder_init = slot_storage_idents.iter().map(|storage_ident| {
        quote! { #storage_ident: core::option::Option::None }
    });
    let complete_builder_ty_generics = {
        let complete = slot_state_idents
            .iter()
            .map(|_| quote! { true })
            .collect::<Vec<_>>();
        generic_argument_tokens(extra_generics.params.iter(), None, &complete)
    };
    let initial_builder_ty_generics = {
        let initial = slot_state_idents
            .iter()
            .map(|_| quote! { false })
            .collect::<Vec<_>>();
        generic_argument_tokens(extra_generics.params.iter(), None, &initial)
    };
    let state_data_binding = if has_state_data {
        let storage_ident = &slot_storage_idents[0];
        Some(quote! {
            let state_data = self.#storage_ident.expect(
                "statum internal error: `state_data` was not set before build",
            );
        })
    } else {
        None
    };
    let field_bindings = field_names.iter().enumerate().map(|(field_idx, field_name)| {
        let storage_ident = &slot_storage_idents[field_idx + usize::from(has_state_data)];
        let message = format!("statum internal error: `{field_name}` was not set before build");
        quote! {
            let #field_name = self.#storage_ident.expect(#message);
        }
    });
    let setters = slot_types.iter().enumerate().map(|(slot_idx, slot_type)| {
        let setter_ident = if has_state_data && slot_idx == 0 {
            format_ident!("state_data")
        } else {
            field_names[slot_idx - usize::from(has_state_data)].clone()
        };
        let available_slot_idents = slot_state_idents
            .iter()
            .enumerate()
            .filter_map(|(idx, ident)| (idx != slot_idx).then_some(ident.clone()))
            .collect::<Vec<_>>();
        let setter_impl_generics_decl =
            builder_generics(&extra_generics, false, &available_slot_idents, false);
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
        let current_ty_generics =
            generic_argument_tokens(extra_generics.params.iter(), None, &current_generics);
        let already_set_generics = slot_state_idents
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
        let already_set_ty_generics =
            generic_argument_tokens(extra_generics.params.iter(), None, &already_set_generics);
        let target_generics = if slot_state_idents.is_empty() {
            extra_ty_args.clone()
        } else {
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
            generic_argument_tokens(extra_generics.params.iter(), None, &generics)
        };
        let assignments = slot_storage_idents.iter().enumerate().map(|(idx, storage_ident)| {
            if idx == slot_idx {
                quote! { #storage_ident: core::option::Option::Some(value) }
            } else {
                quote! { #storage_ident: self.#storage_ident }
            }
        });
        quote! {
            impl #setter_impl_generics #variant_builder_ident #current_ty_generics #setter_where_clause {
                #builder_vis fn #setter_ident(self, value: #slot_type) -> #variant_builder_ident #target_generics {
                    #variant_builder_ident {
                        #(#assignments),*
                    }
                }
            }

            impl #setter_impl_generics #variant_builder_ident #already_set_ty_generics #setter_where_clause {
                #builder_vis fn #setter_ident(self, _value: #already_set_ident) -> Self {
                    self
                }
            }
        }
    });

    quote! {
        #builder_vis struct #variant_builder_ident #builder_defaults {
            #(#struct_fields),*
        }

        #[doc(hidden)]
        #builder_vis enum #already_set_ident {}

        impl #extra_impl_generics #machine_state_ty #extra_where_clause {
            #builder_vis fn builder() -> #variant_builder_ident #initial_builder_ty_generics {
                #variant_builder_ident {
                    #(#builder_init),*
                }
            }
        }
        #(#setters)*

        impl #extra_impl_generics #variant_builder_ident #complete_builder_ty_generics #extra_where_clause {
            #builder_vis fn build(self) -> #machine_state_ty {
                #state_data_binding
                #(#field_bindings)*
                #struct_initialization
            }
        }
    }
}
