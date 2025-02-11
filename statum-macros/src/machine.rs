use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use syn::{Attribute, Generics, Ident, ItemStruct};

use crate::{get_state_enum_map, EnumInfo, StateFilePath};

// Structure to store metadata about a struct
#[derive(Debug, Clone)]
pub struct MachineInfo {
    pub name: String,
    pub vis: String,
    pub derives: Vec<String>,
    pub fields: Vec<MachineField>,
    pub file_path: MachinePath,
    pub generics: String,
}

// Structure to store each field in the struct
#[derive(Debug, Clone)]
pub struct MachineField {
    pub name: String,
    pub field_type: String,
}

// Type-safe wrapper for struct names
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct MachinePath(pub String);

impl From<&Ident> for MachinePath {
    fn from(ident: &Ident) -> Self {
        Self(ident.to_string())
    }
}

// Global storage for all `#[machine]` structs
static MACHINE_MAP: OnceLock<RwLock<HashMap<MachinePath, MachineInfo>>> = OnceLock::new();

pub fn get_machine_map() -> &'static RwLock<HashMap<MachinePath, MachineInfo>> {
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
    pub fn from_item_struct(item: &ItemStruct) -> syn::Result<Self> {
        let name = item.ident.to_string();
        let vis = item.vis.to_token_stream().to_string();
        let generics = item.generics.clone().to_token_stream().to_string();
        let derives = item
            .attrs
            .iter()
            .filter_map(extract_derive)
            .flatten()
            .collect();

        // ✅ Ensure that the struct has a generic parameter
        if item.generics.params.is_empty() {
            return Err(syn::Error::new_spanned(
                item,
                "Error: #[machine] structs must have a generic type parameter implementing `State`.",
            ));
        }

        let fields = item
            .fields
            .iter()
            .map(|field| MachineField {
                name: field.ident.as_ref().unwrap().to_string(),
                field_type: field.ty.to_token_stream().to_string(),
            })
            .collect();

        let file_path = std::env::current_dir()
            .expect("Failed to get current directory.")
            .to_string_lossy()
            .to_string()
            .into();

        Ok(Self {
            derives,
            vis,
            name,
            fields,
            file_path,
            generics,
        })
    }
}

/// Convert from `&str` to `MachineName`
impl From<&str> for MachinePath {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Convert from `String` to `MachineName`
impl From<String> for MachinePath {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Convert from `Ident` to `MachineName`
impl From<Ident> for MachinePath {
    fn from(ident: Ident) -> Self {
        Self(ident.to_string())
    }
}

/// Convert from `TokenStream` to `MachineName`
impl From<TokenStream> for MachinePath {
    fn from(token_stream: TokenStream) -> Self {
        Self(token_stream.to_string())
    }
}

/// Convert `MachineName` into a `TokenStream`
impl From<MachinePath> for TokenStream {
    fn from(machine: MachinePath) -> Self {
        let ident = syn::Ident::new(&machine.0, proc_macro2::Span::call_site());
        quote! { #ident }
    }
}

// Convert MachinePath to StatePath
impl From<MachinePath> for StateFilePath {
    fn from(machine: MachinePath) -> Self {
        StateFilePath(machine.0)
    }
}
impl From<&MachinePath> for StateFilePath {
    fn from(machine: &MachinePath) -> Self {
        StateFilePath(machine.0.clone())
    }
}

/// Allow `MachineName` to be used directly in `quote!`
impl ToTokens for MachinePath {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ident = syn::Ident::new(&self.0, proc_macro2::Span::call_site());
        ident.to_tokens(tokens);
    }
}

impl MachineInfo {
    pub fn fields_to_token_stream(&self) -> TokenStream {
        let fields = self.fields.iter().map(|field| {
            let field_ident = format_ident!("{}", field.name);
            quote! { #field_ident: self.#field_ident, }
        });

        quote! {
            #(#fields)*
        }
    }
}

// Generates struct-based metadata implementations
pub fn generate_machine_impls(machine_info: &MachineInfo) -> proc_macro2::TokenStream {
    println!(
        "[generate_machine_impls] Reading machine_map for: {}",
        machine_info.name
    );

    let map = get_machine_map().read().unwrap();
    println!("[generate_machine_impls] Acquired read lock on machine_map.");

    let Some(machine_info) = map.get(&machine_info.file_path) else {
        println!("[generate_machine_impls] Struct not found in machine_map!");
        return quote! { compile_error!("Struct not found in machine_map."); };
    };

    println!("[generate_machine_impls] Found struct info, generating code...");

    let name_ident = format_ident!("{}", machine_info.name);
    let vis = syn::parse_str::<syn::Visibility>(&machine_info.vis).unwrap();
    let derives: Vec<proc_macro2::TokenStream> = machine_info
        .derives
        .iter()
        .map(|d| quote::ToTokens::to_token_stream(&syn::parse_str::<syn::Path>(d).unwrap()))
        .collect();

    println!("[generate_machine_impls] Finished generating struct.");

    let state_enum = machine_info.get_matching_state_enum();
    let generics_str = machine_info.generics.trim(); // Remove extra spaces
    let replaced_generics =
        generics_str.replace(&state_enum.name, &format!("S: {}Trait", state_enum.name));
    let generics =
        syn::parse_str::<Generics>(&replaced_generics).expect("Failed to parse generics.");

    let fields = machine_info.fields.iter().map(|field| {
        let field_ident = format_ident!("{}", field.name);
        let field_ty = syn::parse_str::<syn::Type>(&field.field_type).unwrap();
        quote! { #vis #field_ident: #field_ty }
    });

    let struct_token_stream = quote! {
        #[derive(#(#derives),*)]
        #vis struct #name_ident #generics {
            #(#fields),*,
            marker: core::marker::PhantomData<S>,
            state_data: Option<S::Data>,
        }
    };

    quote! {
        #struct_token_stream
    }
}

impl MachineInfo {
    pub fn get_matching_state_enum(&self) -> EnumInfo {
        get_state_enum_map()
            .read()
            .unwrap()
            .get(&self.file_path.clone().into())
            .expect("Failed to read state_enum_map.")
            .clone()
    }
}

pub fn validate_machine_struct(
    item: &ItemStruct,
    machine_info: &MachineInfo,
) -> Option<TokenStream> {
    // Ensure it's applied to a struct
    if !matches!(item, ItemStruct { .. }) {
        return Some(quote! {
            compile_error!("#[machine] must be applied to a struct. Example:

            #[state] enum MyState { ... } 
            #[machine]
            struct Machine<MyState> {
                client: String,
                name: String,
                priority: u8,
            }");
        });
    }

    let matching_state_enum = machine_info.get_matching_state_enum();

    // Ensure the struct has at least one generic type parameter
    if matching_state_enum.name != item.generics.params[0].to_token_stream().to_string() {
        return Some(quote! {
            compile_error!("#[machine] structs must have a generic type parameter of the same name as your #[state] enum. Example:

            #[state] enum MyState { ... } 
            #[machine]
            struct Machine<MyState> {
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
    map.insert(machine_info.file_path.clone(), machine_info.clone());
    println!("[store_machine_struct] Inserted struct into machine_map.");
}
