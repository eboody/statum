use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{GenericParam, Generics, Ident};

use super::context::BuilderContext;
use crate::machine::{extra_generics, extra_type_arguments_tokens, generic_argument_tokens};

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
    let slot_labels = (0..slot_types.len())
        .map(|slot_idx| {
            if has_state_data && slot_idx == 0 {
                "state_data".to_owned()
            } else {
                field_names[slot_idx - usize::from(has_state_data)].to_string()
            }
        })
        .collect::<Vec<_>>();
    let slot_storage_idents = (0..slot_types.len())
        .map(|idx| format_ident!("__statum_slot_{}", idx))
        .collect::<Vec<_>>();
    let slot_state_idents = slot_labels
        .iter()
        .enumerate()
        .map(|(slot_idx, label)| {
            format_ident!(
                "__StatumSlot{}{}Stage",
                slot_idx,
                pascal_case_ident_suffix(label)
            )
        })
        .collect::<Vec<_>>();
    let slot_missing_idents = slot_labels
        .iter()
        .enumerate()
        .map(|(slot_idx, label)| {
            format_ident!(
                "__Statum{}MissingSlot{}{}",
                variant_builder_ident,
                slot_idx,
                pascal_case_ident_suffix(label)
            )
        })
        .collect::<Vec<_>>();
    let slot_set_idents = slot_labels
        .iter()
        .enumerate()
        .map(|(slot_idx, label)| {
            format_ident!(
                "__Statum{}SetSlot{}{}",
                variant_builder_ident,
                slot_idx,
                pascal_case_ident_suffix(label)
            )
        })
        .collect::<Vec<_>>();
    let already_set_ident = format_ident!("__Statum{}AlreadySet", variant_builder_ident);
    let struct_fields = slot_storage_idents
        .iter()
        .zip(slot_types.iter())
        .map(|(storage_ident, slot_type)| {
            quote! { #storage_ident: core::option::Option<#slot_type> }
        })
        .collect::<Vec<_>>();
    let stage_marker_type = if slot_state_idents.is_empty() {
        quote! { () }
    } else {
        quote! { (#(#slot_state_idents),*) }
    };
    let builder_defaults = typestate_builder_generics(
        &extra_generics,
        &slot_state_idents,
        &slot_missing_idents,
        true,
    );
    let builder_init = slot_storage_idents.iter().map(|storage_ident| {
        quote! { #storage_ident: core::option::Option::None }
    });
    let complete_builder_ty_generics = {
        let complete = slot_set_idents
            .iter()
            .map(|slot_set_ident| quote! { #slot_set_ident })
            .collect::<Vec<_>>();
        generic_argument_tokens(extra_generics.params.iter(), None, &complete)
    };
    let initial_builder_ty_generics = {
        let initial = slot_missing_idents
            .iter()
            .map(|slot_missing_ident| quote! { #slot_missing_ident })
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
    let field_bindings = field_names
        .iter()
        .enumerate()
        .map(|(field_idx, field_name)| {
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
        let available_missing_idents = slot_missing_idents
            .iter()
            .enumerate()
            .filter_map(|(idx, ident)| (idx != slot_idx).then_some(ident.clone()))
            .collect::<Vec<_>>();
        let setter_impl_generics_decl =
            typestate_builder_generics(&extra_generics, &available_slot_idents, &available_missing_idents, false);
        let (setter_impl_generics, _, setter_where_clause) =
            setter_impl_generics_decl.split_for_impl();
        let current_generics = slot_state_idents
            .iter()
            .enumerate()
            .map(|(idx, ident)| {
                if idx == slot_idx {
                    let missing_ident = &slot_missing_idents[idx];
                    quote! { #missing_ident }
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
                    quote! { #already_set_ident }
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
                        let set_ident = &slot_set_idents[idx];
                        quote! { #set_ident }
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
                    #(#assignments,)*
                    __statum_builder_stage: core::marker::PhantomData,
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
    let slot_stage_markers =
        slot_missing_idents
            .iter()
            .zip(slot_set_idents.iter())
            .map(|(missing_ident, set_ident)| {
                quote! {
                    #[doc(hidden)]
                    #builder_vis struct #missing_ident;

                    #[doc(hidden)]
                    #builder_vis struct #set_ident;
                }
            });

    quote! {
        #(#slot_stage_markers)*

        #builder_vis struct #variant_builder_ident #builder_defaults {
            #(#struct_fields,)*
            __statum_builder_stage: core::marker::PhantomData<#stage_marker_type>,
        }

        #[doc(hidden)]
        #builder_vis enum #already_set_ident {}

        impl #extra_impl_generics #machine_state_ty #extra_where_clause {
            #builder_vis fn builder() -> #variant_builder_ident #initial_builder_ty_generics {
                #variant_builder_ident {
                    #(#builder_init,)*
                    __statum_builder_stage: core::marker::PhantomData,
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

fn typestate_builder_generics(
    extra_generics: &Generics,
    slot_state_idents: &[Ident],
    slot_missing_idents: &[Ident],
    default_slots: bool,
) -> Generics {
    let mut generics = Generics::default();

    generics
        .params
        .extend(extra_generics.params.iter().cloned());
    generics.params.extend(
        slot_state_idents
            .iter()
            .enumerate()
            .map(|(idx, slot_ident)| {
                if default_slots {
                    let missing_ident = &slot_missing_idents[idx];
                    GenericParam::Type(syn::parse_quote!(#slot_ident = #missing_ident))
                } else {
                    GenericParam::Type(syn::parse_quote!(#slot_ident))
                }
            }),
    );

    if !generics.params.is_empty() {
        generics.lt_token = Some(Default::default());
        generics.gt_token = Some(Default::default());
        generics.where_clause = extra_generics.where_clause.clone();
    }

    generics
}

fn pascal_case_ident_suffix(label: &str) -> String {
    let mut suffix = String::new();
    let mut capitalize_next = true;
    let label = label.strip_prefix("r#").unwrap_or(label);

    for ch in label.chars() {
        if ch == '_' {
            capitalize_next = true;
            continue;
        }

        if capitalize_next {
            suffix.extend(ch.to_uppercase());
            capitalize_next = false;
        } else {
            suffix.push(ch);
        }
    }

    suffix
}
