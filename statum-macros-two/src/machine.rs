use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use syn::{Attribute, Generics, Ident, ItemStruct};

// Structure to store metadata about a struct
#[derive(Debug, Clone)]
pub struct MachineInfo {
    pub name: MachineName,
    pub vis: String,
    pub derives: Vec<String>,
    pub fields: Vec<MachineField>,
    //pub generics: Vec<String>,
}

// Structure to store each field in the struct
#[derive(Debug, Clone)]
pub struct MachineField {
    pub name: String,
    pub field_type: String,
}

// Type-safe wrapper for struct names
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct MachineName(pub String);

impl From<&Ident> for MachineName {
    fn from(ident: &Ident) -> Self {
        Self(ident.to_string())
    }
}

// Global storage for all `#[machine]` structs
static MACHINE_MAP: OnceLock<RwLock<HashMap<MachineName, MachineInfo>>> = OnceLock::new();

pub fn get_machine_map() -> &'static RwLock<HashMap<MachineName, MachineInfo>> {
    MACHINE_MAP.get_or_init(|| RwLock::new(HashMap::new()))
}

// Extract derives from `#[derive(Debug, Clone, ...)]`
pub fn extract_derive(attr: &Attribute) -> Option<Vec<String>> {
    if attr.path().is_ident("derive") {
        if let Ok(meta) = attr.meta.require_list() {
            return Some(
                meta.parse_args_with(
                    syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
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

// Extracts machine struct information
impl MachineInfo {
    pub fn from_item_struct(item: &ItemStruct) -> Self {
        let name = MachineName::from(&item.ident);
        let vis = item.vis.to_token_stream().to_string();

        let derives = item
            .attrs
            .iter()
            .filter_map(extract_derive)
            .flatten()
            .collect();

        let fields = item
            .fields
            .iter()
            .map(|field| {
                let name = field
                    .ident
                    .as_ref()
                    .map(|ident| ident.to_string())
                    .unwrap_or_else(|| "_unnamed".to_string());
                let field_type = field.ty.to_token_stream().to_string();

                MachineField { name, field_type }
            })
            .collect();

        Self {
            name,
            vis,
            derives,
            fields,
        }
    }
}

/// Convert from `&str` to `MachineName`
impl From<&str> for MachineName {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Convert from `String` to `MachineName`
impl From<String> for MachineName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Convert from `Ident` to `MachineName`
impl From<Ident> for MachineName {
    fn from(ident: Ident) -> Self {
        Self(ident.to_string())
    }
}

/// Convert from `TokenStream` to `MachineName`
impl From<TokenStream> for MachineName {
    fn from(token_stream: TokenStream) -> Self {
        Self(token_stream.to_string())
    }
}

/// Convert `MachineName` into a `TokenStream`
impl From<MachineName> for TokenStream {
    fn from(machine: MachineName) -> Self {
        let ident = syn::Ident::new(&machine.0, proc_macro2::Span::call_site());
        quote! { #ident }
    }
}

/// Allow `MachineName` to be used directly in `quote!`
impl ToTokens for MachineName {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ident = syn::Ident::new(&self.0, proc_macro2::Span::call_site());
        ident.to_tokens(tokens);
    }
}
// Generates struct-based metadata implementations
pub fn generate_machine_impls(
    machine_name: &MachineName,
    generics: Generics,
) -> proc_macro2::TokenStream {
    println!(
        "[generate_machine_impls] Reading machine_map for: {}",
        machine_name.0
    );

    let map = get_machine_map().read().unwrap();
    println!("[generate_machine_impls] Acquired read lock on machine_map.");

    let Some(machine_info) = map.get(machine_name) else {
        println!("[generate_machine_impls] Struct not found in machine_map!");
        return quote! { compile_error!("Struct not found in machine_map."); };
    };

    println!("[generate_machine_impls] Found struct info, generating code...");

    let name_ident = format_ident!("{}", machine_info.name.0);
    let vis = syn::parse_str::<syn::Visibility>(&machine_info.vis).unwrap();
    let derives: Vec<proc_macro2::TokenStream> = machine_info
        .derives
        .iter()
        .map(|d| quote::ToTokens::to_token_stream(&syn::parse_str::<syn::Path>(d).unwrap()))
        .collect();

    let fields = machine_info.fields.iter().map(|field| {
        let field_ident = format_ident!("{}", field.name);
        let field_ty = syn::parse_str::<syn::Type>(&field.field_type).unwrap();
        quote! { #vis #field_ident: #field_ty }
    });

    println!("[generate_machine_impls] Finished generating struct.");

    let marker = quote! {marker: core::marker::PhantomData<S>};

    if machine_info.fields.is_empty() {
        quote! {
            #[derive(#(#derives),*)]
            #vis struct #name_ident #generics {
                #marker
            }
        }
    } else {
        quote! {
            #[derive(#(#derives),*)]
            #vis struct #name_ident #generics {
                #(#fields),*
                , #marker
            }
        }
    }
}

pub fn validate_machine_struct(item: &ItemStruct) -> Option<TokenStream> {
    // Ensure it's applied to a struct
    if !matches!(item, ItemStruct { .. }) {
        return Some(quote! {
            compile_error!("#[machine] must be applied to a struct. Example:
            
            #[machine]
            struct Machine<S: State> {
                client: String,
                name: String,
                priority: u8,
            }");
        });
    }

    // Ensure the struct has at least one generic type parameter
    if item.generics.params.is_empty() {
        return Some(quote! {
            compile_error!("#[machine] structs must have a generic type parameter implementing `State`. Example:
            
            #[machine]
            struct Machine<S: State> {
                client: String,
                name: String,
                priority: u8,
            }");
        });
    }

    // Ensure the generic parameter implements `State`
    let mut has_state_bound = false;
    for param in &item.generics.params {
        if let syn::GenericParam::Type(type_param) = param {
            for bound in &type_param.bounds {
                if let syn::TypeParamBound::Trait(trait_bound) = bound {
                    if trait_bound.path.is_ident("State") {
                        has_state_bound = true;
                        break;
                    }
                }
            }
        }
    }

    if !has_state_bound {
        return Some(quote! {
            compile_error!("#[machine] structs must have a generic type parameter that implements `State`. Example:
            
            #[machine]
            struct Machine<S: State> {
                client: String,
                name: String,
                priority: u8,
            }");
        });
    }

    None
}

pub fn store_machine_struct(machine_info: &MachineInfo) {
    let mut map = get_machine_map().write().unwrap();
    println!("[store_machine_struct] Acquired write lock on machine_map.");
    map.insert(machine_info.name.clone(), machine_info.clone());
    println!("[store_machine_struct] Inserted struct into machine_map.");
}
