use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Parser, parse_macro_input, DeriveInput, Fields};

fn get_field_info(input: &DeriveInput) -> (Vec<&syn::Ident>, Vec<&syn::Type>) {
    match &input.data {
        syn::Data::Struct(s) => match &s.fields {
            syn::Fields::Named(fields) => {
                let param_names = fields
                    .named
                    .iter()
                    .filter(|f| {
                        f.ident
                            .as_ref()
                            .map_or(false, |i| i != "marker" && i != "state_data")
                    })
                    .map(|f| f.ident.as_ref().unwrap())
                    .collect::<Vec<_>>();
                let param_types = fields
                    .named
                    .iter()
                    .filter(|f| {
                        f.ident
                            .as_ref()
                            .map_or(false, |i| i != "marker" && i != "state_data")
                    })
                    .map(|f| &f.ty)
                    .collect::<Vec<_>>();
                (param_names, param_types)
            }
            _ => panic!("Only named fields are supported"),
        },
        _ => panic!("Only structs are supported"),
    }
}

#[proc_macro_attribute]
pub fn state(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let vis = &input.vis;
    let name = &input.ident;

    let states = match &input.data {
        syn::Data::Enum(data_enum) => data_enum.variants.iter().map(|variant| {
            let variant_ident = &variant.ident;
            let variant_fields = &variant.fields;

            match variant_fields {
                // Handle tuple variant with one field
                Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                    let field_type = &fields.unnamed.first().unwrap().ty;
                    quote! {
                        #vis struct #variant_ident(#field_type);
                        impl #name for #variant_ident {
                            type Data = #field_type;
                            const HAS_DATA: bool = true;

                            fn get_data(&self) -> Option<&Self::Data> {
                                Some(&self.0)
                            }

                            fn get_data_mut(&mut self) -> Option<&mut Self::Data> {
                                Some(&mut self.0)
                            }
                        }
                    }
                }
                // Handle unit variant (no fields)
                Fields::Unit => {
                    quote! {
                        #vis struct #variant_ident;
                        impl #name for #variant_ident {
                            type Data = ();
                            const HAS_DATA: bool = false;

                            fn get_data(&self) -> Option<&Self::Data> {
                                None
                            }

                            fn get_data_mut(&mut self) -> Option<&mut Self::Data> {
                                None
                            }
                        }
                    }
                }
                // Error on other variants
                _ => panic!("Variants must either be unit variants or single-field tuple variants"),
            }
        }),
        _ => panic!("state attribute can only be used on enums"),
    };

    let expanded = quote! {
        #vis trait #name {
            type Data;
            const HAS_DATA: bool;
            fn get_data(&self) -> Option<&Self::Data>;
            fn get_data_mut(&mut self) -> Option<&mut Self::Data>;
        }
        #(#states)*
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn machine(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as DeriveInput);
    let struct_name = &input.ident;
    let state_trait = extract_state_trait(&input);

    // Add marker and optional state data
    if let syn::Data::Struct(ref mut struct_data) = input.data {
        if let syn::Fields::Named(ref mut fields) = struct_data.fields {
            fields.named.push(
                syn::Field::parse_named
                    .parse2(quote! { marker: std::marker::PhantomData<S> })
                    .unwrap(),
            );
            fields.named.push(
                syn::Field::parse_named
                    .parse2(quote! { state_data: Option<S::Data> })
                    .unwrap(),
            );
        }
    }

    let (field_names, field_types) = get_field_info(&input);
    let _field_assigns = quote! {
        #(#field_names: self.#field_names,)*
        marker: std::marker::PhantomData,
    };

    let transition_impl = quote! {
        impl<CurrentState: #state_trait> #struct_name<CurrentState> {
            pub fn transition<NewState: #state_trait>(self) -> #struct_name<NewState>
            /// Transition to a new state without data
            where NewState: #state_trait<Data = ()>
            {
                #struct_name {
                    #(#field_names: self.#field_names,)*
                    marker: std::marker::PhantomData,
                    state_data: None,
                }
            }

            pub fn transition_with<NewState: #state_trait>(self, data: NewState::Data) -> #struct_name<NewState> {
                #struct_name {
                    #(#field_names: self.#field_names,)*
                    marker: std::marker::PhantomData,
                    state_data: Some(data),
                }
            }

            pub fn get_state_data(&self) -> Option<&CurrentState::Data> {
                self.state_data.as_ref()
            }

            pub fn get_state_data_mut(&mut self) -> Option<&mut CurrentState::Data> {
                self.state_data.as_mut()
            }
        }
    };

    let constructor = quote! {
        impl<S: #state_trait> #struct_name<S> {
            pub fn new(#(#field_names: #field_types),*) -> Self {
                Self {
                    #(#field_names,)*
                    marker: std::marker::PhantomData,
                    state_data: None,
                }
            }
        }
    };

    let expanded = quote! {
        #input
        #transition_impl
        #constructor
    };

    TokenStream::from(expanded)
}

fn extract_state_trait(input: &DeriveInput) -> syn::Ident {
    let generics = &input.generics;
    let type_param = generics
        .type_params()
        .next()
        .expect("Struct must have a type parameter");
    let bounds = &type_param.bounds;
    for bound in bounds {
        if let syn::TypeParamBound::Trait(trait_bound) = bound {
            if let Some(segment) = trait_bound.path.segments.last() {
                return segment.ident.clone();
            }
        }
    }
    panic!("Type parameter must have a trait bound")
}
