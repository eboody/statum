use quote::{format_ident, quote};
use syn::{GenericParam, Generics, Ident, Path, Type};

use crate::machine::{builder_generics, generic_argument_tokens};

pub(crate) struct SlotSetterContext<'a> {
    pub(super) builder_ident: &'a Ident,
    pub(super) machine_vis: &'a syn::Visibility,
    pub(super) extra_machine_generics: &'a Generics,
    pub(super) field_names: &'a [Ident],
    pub(super) field_types: &'a [Type],
    pub(super) slot_state_idents: &'a [Ident],
    pub(super) slot_storage_idents: &'a [Ident],
    pub(super) row_lifetime: Option<proc_macro2::TokenStream>,
}

pub(crate) fn slot_state_idents(field_count: usize) -> Vec<Ident> {
    (0..field_count)
        .map(|idx| format_ident!("__STATUM_SLOT_{}_SET", idx))
        .collect()
}

pub(crate) fn slot_storage_idents(field_count: usize) -> Vec<Ident> {
    (0..field_count)
        .map(|idx| format_ident!("__statum_slot_{}", idx))
        .collect()
}

pub(crate) fn field_storage_tokens(
    slot_storage_idents: &[Ident],
    field_types: &[Type],
) -> Vec<proc_macro2::TokenStream> {
    slot_storage_idents
        .iter()
        .zip(field_types.iter())
        .map(|(storage_ident, field_type)| {
            quote! { #storage_ident: core::option::Option<#field_type> }
        })
        .collect()
}

pub(crate) fn field_binding_tokens(
    field_names: &[Ident],
    slot_storage_idents: &[Ident],
) -> Vec<proc_macro2::TokenStream> {
    field_names
        .iter()
        .zip(slot_storage_idents.iter())
        .map(|(field_name, storage_ident)| {
            let message = format!("statum internal error: `{field_name}` was not set before build");
            quote! {
                let #field_name = self.#storage_ident.expect(#message);
            }
        })
        .collect()
}

pub(crate) fn slot_setter_impls<F>(
    context: SlotSetterContext<'_>,
    build_instance: F,
) -> Vec<proc_macro2::TokenStream>
where
    F: Fn(Vec<proc_macro2::TokenStream>) -> proc_macro2::TokenStream,
{
    context
        .field_names
        .iter()
        .zip(context.field_types.iter())
        .enumerate()
        .map(|(slot_idx, (field_name, field_type))| {
            let builder_ident = context.builder_ident;
            let machine_vis = context.machine_vis;
            let available_slot_idents = context
                .slot_state_idents
                .iter()
                .enumerate()
                .filter_map(|(idx, ident)| (idx != slot_idx).then_some(ident.clone()))
                .collect::<Vec<_>>();
            let setter_impl_generics_decl = builder_generics(
                context.extra_machine_generics,
                context.row_lifetime.is_some(),
                &available_slot_idents,
                false,
            );
            let (setter_impl_generics, _, setter_where_clause) =
                setter_impl_generics_decl.split_for_impl();
            let current_generics = context
                .slot_state_idents
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
                context.extra_machine_generics.params.iter(),
                context.row_lifetime.clone(),
                &current_generics,
            );
            let target_generics = context
                .slot_state_idents
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
            let target_ty_generics = generic_argument_tokens(
                context.extra_machine_generics.params.iter(),
                context.row_lifetime.clone(),
                &target_generics,
            );
            let assignments = context
                .slot_storage_idents
                .iter()
                .enumerate()
                .map(|(idx, storage_ident)| {
                    if idx == slot_idx {
                        quote! { #storage_ident: core::option::Option::Some(value) }
                    } else {
                        quote! { #storage_ident: self.#storage_ident }
                    }
                })
                .collect::<Vec<_>>();
            let instance = build_instance(assignments);

            quote! {
                impl #setter_impl_generics #builder_ident #current_ty_generics #setter_where_clause {
                    #machine_vis fn #field_name(self, value: #field_type) -> #builder_ident #target_ty_generics {
                        #instance
                    }
                }
            }
        })
        .collect()
}

pub(crate) fn rebuild_attempt_tokens(
    validator_fn_ident: &Ident,
    variant_ident: &Ident,
    matched: bool,
) -> proc_macro2::TokenStream {
    quote! {
        statum::RebuildAttempt {
            validator: stringify!(#validator_fn_ident),
            target_state: stringify!(#variant_ident),
            matched: #matched,
            reason_key: core::option::Option::None,
            message: core::option::Option::None,
        }
    }
}

pub(crate) fn failed_rebuild_attempt_with_rejection_tokens(
    validator_fn_ident: &Ident,
    variant_ident: &Ident,
) -> proc_macro2::TokenStream {
    quote! {
        statum::RebuildAttempt {
            validator: stringify!(#validator_fn_ident),
            target_state: stringify!(#variant_ident),
            matched: false,
            reason_key: core::option::Option::Some(__statum_rejection.reason_key),
            message: __statum_rejection.message.clone(),
        }
    }
}

pub(crate) fn machine_builder_path_tokens(
    machine_path: &Path,
    machine_generics: &Generics,
    variant_ident: &Ident,
) -> proc_macro2::TokenStream {
    let state_marker_path = machine_scoped_item_path(machine_path, variant_ident);
    let mut args = vec![quote! { #state_marker_path }];
    args.extend(
        machine_generics
            .params
            .iter()
            .skip(1)
            .map(generic_argument_token),
    );
    quote! { #machine_path::<#(#args),*> }
}

pub(crate) fn machine_state_variant_path_tokens(
    machine_module_path: &Path,
    machine_generics: &Generics,
    variant_ident: &Ident,
) -> proc_macro2::TokenStream {
    let extra_args = machine_generics
        .params
        .iter()
        .skip(1)
        .map(generic_argument_token)
        .collect::<Vec<_>>();
    if extra_args.is_empty() {
        quote! { #machine_module_path::SomeState::#variant_ident }
    } else {
        quote! { #machine_module_path::SomeState::<#(#extra_args),*>::#variant_ident }
    }
}

pub(crate) fn generic_usage_marker_tokens(generics: &Generics) -> proc_macro2::TokenStream {
    let usages = generics
        .params
        .iter()
        .map(|param| match param {
            GenericParam::Lifetime(lifetime) => {
                let lifetime = &lifetime.lifetime;
                quote! { &#lifetime () }
            }
            GenericParam::Type(ty) => {
                let ident = &ty.ident;
                quote! { #ident }
            }
            GenericParam::Const(const_param) => {
                let ident = &const_param.ident;
                quote! { [(); #ident] }
            }
        })
        .collect::<Vec<_>>();

    if usages.len() == 1 {
        usages.into_iter().next().unwrap()
    } else {
        quote! { (#(#usages),*) }
    }
}

pub(crate) fn machine_scoped_item_path(machine_path: &Path, item_ident: &Ident) -> Path {
    let mut scoped_path = machine_path.clone();
    if let Some(last_segment) = scoped_path.segments.last_mut() {
        last_segment.ident = item_ident.clone();
    }
    scoped_path
}

pub(crate) fn trait_extra_generic_argument_tokens(
    extra_generics: &Generics,
) -> proc_macro2::TokenStream {
    let extra_args = extra_generics
        .params
        .iter()
        .map(generic_argument_token)
        .collect::<Vec<_>>();
    if extra_args.is_empty() {
        quote! {}
    } else {
        quote! {, #(#extra_args),* }
    }
}

pub(crate) fn prefixed_generics_declaration_tokens(
    first_param: &str,
    extra_generics: &Generics,
) -> proc_macro2::TokenStream {
    let first_ident = format_ident!("{}", first_param);
    let extra_params = extra_generics.params.iter().cloned().collect::<Vec<_>>();
    if extra_params.is_empty() {
        quote! { <#first_ident> }
    } else {
        quote! { <#first_ident, #(#extra_params),*> }
    }
}

pub(crate) fn prefixed_generics_argument_tokens<'a>(
    first_arg: proc_macro2::TokenStream,
    extra_params: impl Iterator<Item = &'a GenericParam>,
) -> proc_macro2::TokenStream {
    let mut args = vec![first_arg];
    args.extend(extra_params.map(generic_argument_token));
    quote! { <#(#args),*> }
}

pub(crate) fn merged_where_clause_tokens(
    extra_where_clause: Option<&syn::WhereClause>,
    additional_predicates: Vec<proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    let mut predicates = extra_where_clause
        .into_iter()
        .flat_map(|where_clause| {
            where_clause
                .predicates
                .iter()
                .map(|predicate| quote! { #predicate })
        })
        .collect::<Vec<_>>();
    predicates.extend(additional_predicates);

    if predicates.is_empty() {
        quote! {}
    } else {
        quote! { where #(#predicates),* }
    }
}

fn generic_argument_token(param: &GenericParam) -> proc_macro2::TokenStream {
    match param {
        GenericParam::Lifetime(lifetime) => {
            let lifetime = &lifetime.lifetime;
            quote! { #lifetime }
        }
        GenericParam::Type(ty) => {
            let ident = &ty.ident;
            quote! { #ident }
        }
        GenericParam::Const(const_param) => {
            let ident = &const_param.ident;
            quote! { #ident }
        }
    }
}
