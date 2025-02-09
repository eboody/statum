use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use std::{
    collections::HashMap,
    sync::{OnceLock, RwLock},
};
use syn::{Attribute, Fields, Ident, ItemEnum, Path};

// Structure to hold extracted enum data
#[derive(Clone, Debug)]
#[allow(unused)]
pub struct EnumInfo {
    pub derives: Vec<String>,
    pub vis: String,
    pub name: StateEnumName,
    pub variants: Vec<VariantInfo>,
}

#[derive(Clone, Debug)]
pub struct VariantInfo {
    pub name: String,
    pub data_type: Option<String>,
}

// Type-safe wrapper around an enum name
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct StateEnumName(pub String);

impl AsRef<str> for StateEnumName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Convert `StateEnumName` into a `TokenStream` (for procedural macros)
impl ToTokens for StateEnumName {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ident = syn::Ident::new(&self.0, proc_macro2::Span::call_site());
        ident.to_tokens(tokens);
    }
}

/// Convert from `&str` to `StateEnumName`
impl From<&str> for StateEnumName {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Convert from `String` to `StateEnumName`
impl From<String> for StateEnumName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Convert from `Ident` (Rust identifiers) to `StateEnumName`
impl From<Ident> for StateEnumName {
    fn from(ident: Ident) -> Self {
        Self(ident.to_string())
    }
}

/// Convert from `&Ident` to `StateEnumName`
impl From<&Ident> for StateEnumName {
    fn from(ident: &Ident) -> Self {
        Self(ident.to_string())
    }
}

/// Convert from `TokenStream` to `StateEnumName`
impl From<TokenStream> for StateEnumName {
    fn from(token_stream: TokenStream) -> Self {
        Self(token_stream.to_string())
    }
}

/// Convert `StateEnumName` into a `TokenStream`
impl From<StateEnumName> for TokenStream {
    fn from(state: StateEnumName) -> Self {
        let ident = syn::Ident::new(&state.0, proc_macro2::Span::call_site());
        quote! { #ident }
    }
}

/// Convert `EnumInfo` into a `TokenStream`
impl ToTokens for EnumInfo {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = &self.name;
        let variants = self.variants.iter().map(|v| {
            let var_name = syn::Ident::new(&v.name, proc_macro2::Span::call_site());
            match &v.data_type {
                Some(ty) => {
                    let ty = syn::parse_str::<syn::Type>(ty).unwrap();
                    quote! { #var_name(#ty) }
                }
                None => quote! { #var_name },
            }
        });

        let expanded = quote! {
            pub enum #name {
                #(#variants),*
            }
        };

        tokens.extend(expanded);
    }
}

// Global storage for `#[state]` enums

static STATE_ENUMS: OnceLock<RwLock<HashMap<StateEnumName, EnumInfo>>> = OnceLock::new();

pub fn get_state_enum_map() -> &'static RwLock<HashMap<StateEnumName, EnumInfo>> {
    STATE_ENUMS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Extracts `#[derive(...)]` attributes from an enum
pub fn extract_derive(attr: &Attribute) -> Option<Vec<String>> {
    if attr.path().is_ident("derive") {
        if let Ok(meta) = attr.meta.require_list() {
            return Some(
                meta.parse_args_with(
                    syn::punctuated::Punctuated::<Path, syn::Token![,]>::parse_terminated,
                )
                .ok()?
                .iter()
                .map(|p| p.to_token_stream().to_string())
                .collect(),
            );
        }
    }
    None
}

impl EnumInfo {
    /// Constructs `EnumInfo` from a parsed `ItemEnum`
    pub fn from_item_enum(item: &ItemEnum) -> Self {
        let name = StateEnumName::from(&item.ident);

        // Extract visibility (`pub` or empty string if private)
        let vis = item.vis.to_token_stream().to_string();

        // Extract `#[derive(...)]` attributes
        let derives = item
            .attrs
            .iter()
            .filter_map(extract_derive) // Uses `extract_derive()` from before
            .flatten()
            .collect();

        // Extract variants
        let variants = item
            .variants
            .iter()
            .map(|variant| {
                let name = variant.ident.to_string();
                let data_type = match &variant.fields {
                    Fields::Unnamed(fields) if fields.unnamed.len() == 1 => Some(
                        fields
                            .unnamed
                            .first()
                            .unwrap()
                            .ty
                            .to_token_stream()
                            .to_string(),
                    ),
                    _ => None,
                };
                VariantInfo { name, data_type }
            })
            .collect();

        Self {
            derives,
            vis,
            name,
            variants,
        }
    }
}
pub fn generate_state_impls(enum_name: &StateEnumName) -> proc_macro2::TokenStream {
    let map = get_state_enum_map().read().unwrap();
    let Some(enum_info) = map.get(enum_name) else {
        return quote! { compile_error!("Enum not found in state_enum_map."); };
    };

    let name_ident = format_ident!("{}", enum_info.name.as_ref());
    let vis = syn::parse_str::<syn::Visibility>(&enum_info.vis).unwrap();

    let derives: Vec<proc_macro2::TokenStream> = enum_info
        .derives
        .iter()
        .map(|d| quote::ToTokens::to_token_stream(&syn::parse_str::<syn::Path>(d).unwrap()))
        .collect();

    // Generate one struct and implementation per variant
    let variant_structs = enum_info.variants.iter().map(|variant| {
        let variant_name = format_ident!("{}", variant.name);

        match &variant.data_type {
            // Handle tuple variants
            Some(field_type) => {
                let field_ty = syn::parse_str::<syn::Type>(field_type).unwrap();
                quote! {
                    #[derive(#(#derives),*)]
                    #vis struct #variant_name(pub #field_ty);

                    impl #variant_name {
                        pub fn get_data(&self) -> &#field_ty {
                            &self.0
                        }

                        pub fn get_data_mut(&mut self) -> &mut #field_ty {
                            &mut self.0
                        }
                    }

                    impl #name_ident for #variant_name {}

                }
            }
            // Handle unit variants
            None => {
                quote! {
                    #[derive(#(#derives),*)]
                    #vis struct #variant_name;

                    impl #name_ident for #variant_name {}
                }
            }
        }
    });

    let state_trait = quote! {
        #vis trait #name_ident {}
    };

    // Generate the trait definition and include all variant structs
    quote! {
        #state_trait

        #(#variant_structs)*
    }
}
pub fn validate_state_enum(item: &ItemEnum) -> Option<TokenStream> {
    // Ensure it's applied to an enum
    if !matches!(item, ItemEnum { .. }) {
        return Some(quote! {
            compile_error!("#[state] must be applied to an enum. Example:
            
            #[state]
            enum ExampleState {
                Draft,
                InProgress(String),
                Complete,
            }");
        });
    }

    // Ensure the enum has at least one variant
    if item.variants.is_empty() {
        return Some(quote! {
            compile_error!("#[state] enums must have at least one variant.");
        });
    }

    // Ensure all variants are unit or single-field tuples
    for variant in &item.variants {
        if !matches!(&variant.fields, Fields::Unit | Fields::Unnamed(_)) {
            let var_name = variant.ident.to_string();
            return Some(quote! {
                compile_error!(concat!(
                    "Invalid variant '", #var_name, "' in #[state] enum. ",
                    "Variants must be unit or single-field tuple variants. Example:\n\n",
                    "enum ExampleState {\n",
                    "    Draft,\n",
                    "    InProgress(String),\n",
                    "    Complete,\n",
                    "}"
                ));
            });
        }
    }

    None
}

pub fn store_state_enum(enum_info: &EnumInfo) {
    let mut map = get_state_enum_map().write().unwrap();
    println!("[store_state_enum] Acquired write lock on state_enum_map.");
    map.insert(enum_info.name.clone(), enum_info.clone());
    println!("[store_state_enum] Inserted enum into state_enum_map.");
}
