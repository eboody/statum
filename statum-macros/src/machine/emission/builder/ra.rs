use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use super::context::BuilderContext;
use crate::machine::{extra_generics, extra_type_arguments_tokens};

pub(in crate::machine::emission) fn rust_analyzer_builder_tokens(
    context: &BuilderContext<'_>,
    variant_builder_ident: &Ident,
    data_type: Option<&syn::Type>,
    machine_state_ty: &TokenStream,
) -> TokenStream {
    let builder_vis = context.builder_vis;
    let field_names = context.field_names;
    let field_types = context.field_types;
    let extra_generics = extra_generics(context.machine_generics);
    let extra_ty_args = extra_type_arguments_tokens(context.machine_generics);
    let (extra_impl_generics, _, extra_where_clause) = extra_generics.split_for_impl();
    let builder_generics = extra_generics.clone();
    let (builder_impl_generics, builder_ty_generics, builder_where_clause) =
        builder_generics.split_for_impl();
    let state_data_method = data_type.map(|parsed_data_type| {
        quote! {
            #builder_vis fn state_data(self, _data: #parsed_data_type) -> Self {
                self
            }
        }
    });

    quote! {
        #builder_vis struct #variant_builder_ident #builder_generics;

        impl #builder_impl_generics #variant_builder_ident #builder_ty_generics #builder_where_clause {
            #state_data_method
            #(#builder_vis fn #field_names(self, _value: #field_types) -> Self { self })*

            #builder_vis fn build(self) -> #machine_state_ty {
                panic!("statum rust-analyzer shim: builder values are not constructed at runtime")
            }
        }

        impl #extra_impl_generics #machine_state_ty #extra_where_clause {
            #builder_vis fn builder() -> #variant_builder_ident #extra_ty_args {
                #variant_builder_ident
            }
        }
    }
}
