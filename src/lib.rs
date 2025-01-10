use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::Parser, parse_macro_input, punctuated::Punctuated, Data, DeriveInput, Fields, Path,
    Token,
};

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
                            .is_some_and(|i| i != "marker" && i != "state_data")
                    })
                    .map(|f| f.ident.as_ref().unwrap())
                    .collect::<Vec<_>>();
                let param_types = fields
                    .named
                    .iter()
                    .filter(|f| {
                        f.ident
                            .as_ref()
                            .is_some_and(|i| i != "marker" && i != "state_data")
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

    // Analyze user-supplied #[derive(...)] to detect which traits they want
    let (user_derives, wants_serialize, wants_deserialize, wants_debug, _wants_clone) =
        analyze_user_derives(&input.attrs);

    // We'll accumulate any trait bounds we need in "trait_bounds".
    let mut trait_bounds = vec![];

    // If the user derived Debug, we add std::fmt::Debug as a bound.
    if wants_debug {
        trait_bounds.push(quote!(std::fmt::Debug));
    }

    // Only add serde bounds if our crate's "serde" feature is enabled.
    #[cfg(feature = "serde")]
    {
        if wants_serialize {
            trait_bounds.push(quote!(serde::Serialize));
        }
        if wants_deserialize {
            trait_bounds.push(quote!(serde::de::DeserializeOwned));
        }
    }

    let trait_bounds = if trait_bounds.is_empty() {
        quote!()
    } else {
        quote!(: #(#trait_bounds +)*)
    };

    // We'll replicate all user-specified derives on each generated variant struct.
    let replicate_derives = if user_derives.is_empty() {
        quote!()
    } else {
        quote! {
            #[derive(#(#user_derives),*)]
        }
    };

    // Convert each enum variant into a separate struct with an impl that ties back to the "State" trait.
    let states = match &input.data {
        Data::Enum(data_enum) => data_enum.variants.iter().map(|variant| {
            let variant_ident = &variant.ident;
            let variant_fields = &variant.fields;
            match variant_fields {
                // Single-field tuple variant
                Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                    let field_type = &fields.unnamed.first().unwrap().ty;
                    quote! {
                        #replicate_derives
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
                // Unit variant
                Fields::Unit => {
                    quote! {
                        #replicate_derives
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
                _ => panic!("Variants must be unit or single-field tuple variants"),
            }
        }),
        _ => {
            return syn::Error::new_spanned(&input.ident, "#[state] can only be used on an enum")
                .to_compile_error()
                .into();
        }
    };

    let expanded = quote! {
        // The trait for this "state" enum
        #vis trait #name {
            type Data #trait_bounds;
            const HAS_DATA: bool;
            fn get_data(&self) -> Option<&Self::Data>;
            fn get_data_mut(&mut self) -> Option<&mut Self::Data>;
        }

        // One struct + impl per variant
        #(#states)*
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn machine(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as DeriveInput);
    let struct_name = &input.ident;
    let state_trait = extract_state_trait(&input);

    // Insert "marker" and "state_data" fields into the user's struct.
    if let syn::Data::Struct(ref mut struct_data) = input.data {
        if let syn::Fields::Named(ref mut fields) = struct_data.fields {
            fields.named.push(
                syn::Field::parse_named
                    .parse2(quote! { marker: core::marker::PhantomData<S> })
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

    let transition_impl = quote! {
        impl<CurrentState: #state_trait> #struct_name<CurrentState> {
            pub fn transition<NewState: #state_trait>(self) -> #struct_name<NewState>
            where
                NewState: #state_trait<Data = ()>
            {
                #struct_name {
                    #(#field_names: self.#field_names,)*
                    marker: core::marker::PhantomData,
                    state_data: None,
                }
            }

            pub fn transition_with<NewState: #state_trait>(self, data: NewState::Data) -> #struct_name<NewState> {
                #struct_name {
                    #(#field_names: self.#field_names,)*
                    marker: core::marker::PhantomData,
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
                    marker: core::marker::PhantomData,
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
    panic!("Type parameter must have a trait bound");
}

fn analyze_user_derives(attrs: &[syn::Attribute]) -> (Vec<Path>, bool, bool, bool, bool) {
    let mut user_derives = Vec::new();
    let mut wants_serialize = false;
    let mut wants_deserialize = false;
    let mut wants_debug = false;
    let mut wants_clone = false;

    // Parse `#[derive(...)]` with syn 2.0
    for attr in attrs {
        if attr.path().is_ident("derive") {
            if let Ok(paths) = attr.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)
            {
                for path in paths {
                    if let Some(ident) = path.get_ident() {
                        match ident.to_string().as_str() {
                            "Serialize" => wants_serialize = true,
                            "Deserialize" => wants_deserialize = true,
                            "Debug" => wants_debug = true,
                            "Clone" => wants_clone = true,
                            _ => {}
                        }
                    }
                    user_derives.push(path);
                }
            }
        }
    }

    (
        user_derives,
        wants_serialize,
        wants_deserialize,
        wants_debug,
        wants_clone,
    )
}
