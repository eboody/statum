use proc_macro2::TokenStream;
use quote::quote;
use syn::{GenericParam, Generics, Ident};

pub(crate) fn extra_generics(generics: &Generics) -> Generics {
    let mut extra = generics.clone();
    extra.params = generics.params.iter().skip(1).cloned().collect();
    if extra.params.is_empty() {
        extra.lt_token = None;
        extra.gt_token = None;
        extra.where_clause = None;
    }
    extra
}

pub(crate) fn extra_type_arguments_tokens(generics: &Generics) -> TokenStream {
    generic_argument_tokens(generics.params.iter().skip(1), None, &[])
}

pub(crate) fn machine_type_with_state(
    machine_ty: TokenStream,
    generics: &Generics,
    state_ty: TokenStream,
) -> TokenStream {
    let mut args = vec![state_ty];
    args.extend(
        generics
            .params
            .iter()
            .skip(1)
            .map(generic_argument_token),
    );

    quote! { #machine_ty<#(#args),*> }
}

pub(crate) fn builder_generics(
    extra_generics: &Generics,
    include_row_lifetime: bool,
    slot_state_idents: &[Ident],
    default_slots: bool,
) -> Generics {
    let mut generics = Generics::default();

    if include_row_lifetime {
        generics.params.push(syn::parse_quote!('__statum_row));
    }

    generics.params.extend(extra_generics.params.iter().cloned());
    generics.params.extend(slot_state_idents.iter().map(|slot_ident| {
        if default_slots {
            syn::GenericParam::Const(syn::parse_quote!(const #slot_ident: bool = false))
        } else {
            syn::GenericParam::Const(syn::parse_quote!(const #slot_ident: bool))
        }
    }));

    if !generics.params.is_empty() {
        generics.lt_token = Some(Default::default());
        generics.gt_token = Some(Default::default());
        generics.where_clause = extra_generics.where_clause.clone();
    }

    generics
}

pub(crate) fn generic_argument_tokens<'a>(
    params: impl Iterator<Item = &'a GenericParam>,
    row_lifetime: Option<TokenStream>,
    slot_values: &[TokenStream],
) -> TokenStream {
    let mut args = Vec::new();
    if let Some(row_lifetime) = row_lifetime {
        args.push(row_lifetime);
    }
    args.extend(params.map(generic_argument_token));
    args.extend(slot_values.iter().cloned());

    if args.is_empty() {
        quote! {}
    } else {
        quote! { <#(#args),*> }
    }
}

fn generic_argument_token(param: &GenericParam) -> TokenStream {
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
