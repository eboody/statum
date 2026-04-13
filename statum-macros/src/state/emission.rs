use quote::{format_ident, quote};

use super::{EnumInfo, ParsedVariantShape};

pub fn generate_state_impls(enum_info: &EnumInfo) -> proc_macro2::TokenStream {
    let state_trait_ident = enum_info.get_trait_name();
    let parsed_enum = match enum_info.parse() {
        Ok(parsed) => parsed,
        Err(err) => return err,
    };
    let vis = parsed_enum.vis;
    let derive_tokens = parsed_enum
        .derives
        .iter()
        .map(quote::ToTokens::to_token_stream)
        .collect::<Vec<_>>();

    let mut variant_structs = Vec::with_capacity(enum_info.variants.len());
    for variant in parsed_enum.variants {
        let variant_name = format_ident!("{}", variant.name);
        let variant_derives = if derive_tokens.is_empty() {
            quote! {}
        } else {
            quote! { #[derive(#(#derive_tokens),*)] }
        };

        let tokens = match &variant.shape {
            ParsedVariantShape::Unit => {
                quote! {
                    #variant_derives
                    #vis struct #variant_name;

                    impl #state_trait_ident for #variant_name {
                        type Data = ();
                    }

                    impl statum::StateMarker for #variant_name {
                        type Data = ();
                    }

                    impl statum::UnitState for #variant_name {}
                }
            }
            ParsedVariantShape::Tuple { data_type } => {
                quote! {
                    #variant_derives
                    #vis struct #variant_name (pub #data_type);

                    impl #state_trait_ident for #variant_name {
                        type Data = #data_type;
                    }

                    impl statum::StateMarker for #variant_name {
                        type Data = #data_type;
                    }

                    impl statum::DataState for #variant_name {}
                }
            }
            ParsedVariantShape::Named {
                data_struct_ident,
                fields,
            } => {
                let payload_fields = fields.iter().map(|field| {
                    let field_ident = &field.ident;
                    let field_type = &field.field_type;
                    quote! { pub #field_ident: #field_type }
                });

                quote! {
                    #variant_derives
                    #vis struct #data_struct_ident {
                        #(#payload_fields),*
                    }

                    #variant_derives
                    #vis struct #variant_name (pub #data_struct_ident);

                    impl #state_trait_ident for #variant_name {
                        type Data = #data_struct_ident;
                    }

                    impl statum::StateMarker for #variant_name {
                        type Data = #data_struct_ident;
                    }

                    impl statum::DataState for #variant_name {}
                }
            }
        };
        variant_structs.push(tokens);
    }

    let state_trait = quote! {
        #enum_info
        #vis trait #state_trait_ident {
            type Data;
        }
    };

    let uninitialized_state_name = format_ident!("Uninitialized{}", enum_info.name);

    let uninitialized_state = quote! {
        pub struct #uninitialized_state_name;

        impl #state_trait_ident for #uninitialized_state_name {
            type Data = ();
        }

        impl statum::StateMarker for #uninitialized_state_name {
            type Data = ();
        }

        impl statum::UnitState for #uninitialized_state_name {}
    };

    quote! {
        #state_trait

        #(#variant_structs)*

        #uninitialized_state
    }
}
