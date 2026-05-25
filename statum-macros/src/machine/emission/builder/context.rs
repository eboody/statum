use proc_macro2::TokenStream;
use quote::quote;
use syn::{Generics, Ident, Visibility};

use crate::state::{ParsedVariantInfo, ParsedVariantShape};

pub(in crate::machine::emission) struct BuilderContext<'a> {
    pub(in crate::machine::emission) machine_ident: &'a Ident,
    pub(in crate::machine::emission) machine_generics: &'a Generics,
    pub(in crate::machine::emission) builder_vis: &'a Visibility,
    pub(in crate::machine::emission) field_names: &'a [Ident],
    pub(in crate::machine::emission) field_types: &'a [syn::Type],
    pub(in crate::machine::emission) use_ra_shim: bool,
}

pub(in crate::machine::emission) fn variant_payload_type(
    variant: &ParsedVariantInfo,
) -> Option<syn::Type> {
    match &variant.shape {
        ParsedVariantShape::Unit => None,
        ParsedVariantShape::Tuple { data_type } => Some(*data_type.clone()),
        ParsedVariantShape::Named {
            data_struct_ident, ..
        } => Some(syn::parse_quote!(#data_struct_ident)),
    }
}

pub(in crate::machine::emission) fn machine_struct_initialization(
    context: &BuilderContext<'_>,
    has_state_data: bool,
) -> TokenStream {
    let machine_ident = context.machine_ident;
    let field_names = context.field_names;
    let state_data = if has_state_data {
        quote! { state_data }
    } else {
        quote! { state_data: () }
    };

    if !field_names.is_empty() {
        quote! {
            #machine_ident {
                marker: core::marker::PhantomData,
                #state_data,
                #(#field_names,)*
            }
        }
    } else {
        quote! {
            #machine_ident {
                marker: core::marker::PhantomData,
                #state_data,
            }
        }
    }
}
