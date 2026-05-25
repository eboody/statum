use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::machine::machine_type_with_state;
use crate::state::{ParsedEnumInfo, ParsedVariantInfo};

use super::super::metadata::{ParsedMachineInfo, is_rust_analyzer};
use super::builder::{
    BuilderContext, machine_struct_initialization, rust_analyzer_builder_tokens,
    typestate_builder_tokens, variant_payload_type,
};
use crate::machine::MachineInfo;

impl MachineInfo {
    pub fn generate_builder_methods(
        &self,
        parsed_machine: &ParsedMachineInfo,
        parsed_state: &ParsedEnumInfo,
    ) -> TokenStream {
        let parsed_fields = parsed_machine.field_idents_and_types();
        let field_names = parsed_fields
            .iter()
            .map(|(field_ident, _)| field_ident.clone())
            .collect::<Vec<_>>();
        let field_types = parsed_fields
            .iter()
            .map(|(_, field_ty)| field_ty.clone())
            .collect::<Vec<_>>();

        let machine_ident = format_ident!("{}", self.name);
        let builder_context = BuilderContext {
            machine_ident: &machine_ident,
            machine_generics: &parsed_machine.generics,
            builder_vis: &parsed_machine.vis,
            field_names: &field_names,
            field_types: &field_types,
            use_ra_shim: is_rust_analyzer(),
        };
        let builder_methods = parsed_state
            .variants
            .iter()
            .map(|variant| generate_variant_builder_tokens(&builder_context, variant));

        quote! {
            #(#builder_methods)*
        }
    }
}

fn generate_variant_builder_tokens(
    context: &BuilderContext<'_>,
    variant: &ParsedVariantInfo,
) -> TokenStream {
    let variant_ident = format_ident!("{}", variant.name);
    let variant_builder_ident = format_ident!("{}{}Builder", context.machine_ident, variant.name);
    let data_type = variant_payload_type(variant);
    generate_custom_builder_tokens(
        context,
        &variant_ident,
        &variant_builder_ident,
        data_type.as_ref(),
    )
}

fn generate_custom_builder_tokens(
    context: &BuilderContext<'_>,
    variant_ident: &syn::Ident,
    variant_builder_ident: &syn::Ident,
    data_type: Option<&syn::Type>,
) -> TokenStream {
    let machine_ident = context.machine_ident;
    let machine_state_ty = machine_type_with_state(
        quote! { #machine_ident },
        context.machine_generics,
        quote! { #variant_ident },
    );
    let struct_initialization = machine_struct_initialization(context, data_type.is_some());

    if context.use_ra_shim {
        rust_analyzer_builder_tokens(context, variant_builder_ident, data_type, &machine_state_ty)
    } else {
        typestate_builder_tokens(
            context,
            variant_builder_ident,
            data_type,
            &machine_state_ty,
            struct_initialization,
        )
    }
}
