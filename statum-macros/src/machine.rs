use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use syn::{Attribute, Generics, Ident, ItemStruct};

use crate::{read_state_enum_map, EnumInfo, StateFilePath};

impl<T: ToString> From<T> for MachinePath {
    fn from(value: T) -> Self {
        Self(value.to_string())
    }
}

// Convert MachinePath to StatePath
impl From<MachinePath> for StateFilePath {
    fn from(machine: MachinePath) -> Self {
        StateFilePath(machine.0)
    }
}

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


// Global storage for all `#[machine]` structs
static MACHINE_MAP: OnceLock<RwLock<HashMap<MachinePath, MachineInfo>>> = OnceLock::new();

pub fn get_machine_map() -> &'static RwLock<HashMap<MachinePath, MachineInfo>> {
    MACHINE_MAP.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn read_machine_map() -> HashMap<MachinePath, MachineInfo> {
    get_machine_map().read().unwrap().clone()
}

// Extract derives from `#[derive(Debug, Clone, ...)]`

pub fn extract_derive(attr: &Attribute) -> Option<Vec<String>> {
    if !attr.path().is_ident("derive") {
        return None;
    }
    attr.meta.require_list().ok()?.parse_args_with(
        syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
    ).ok()
    .map(|punctuated| punctuated.iter().map(|p| p.to_token_stream().to_string()).collect())
}


// Extracts machine struct information
impl MachineInfo {
    pub fn from_item_struct(item: &ItemStruct) -> syn::Result<Self> {
        let fields = item.fields.iter().filter_map(|field| {
            field.ident.as_ref().map(|ident| MachineField {
                name: ident.to_string(),
                field_type: field.ty.to_token_stream().to_string(),
            })
        }).collect();

        if item.generics.params.is_empty() {
            return Err(syn::Error::new_spanned(item, 
                "Error: #[machine] structs must have a generic type parameter implementing `State`."));
        }

        Ok(Self {
            name: item.ident.to_string(),
            vis: item.vis.to_token_stream().to_string(),
            derives: item.attrs.iter().filter_map(extract_derive).flatten().collect(),
            fields,
            file_path: std::env::current_dir()
                .expect("Failed to get current directory.")
                .to_string_lossy()
                .to_string()
                .into(),
            generics: item.generics.to_token_stream().to_string(),
        })
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
    if let Some(machine_info) = get_machine_map().read().unwrap().get(&machine_info.file_path) {
        let name_ident = format_ident!("{}", machine_info.name);
        let generics = parse_generics(machine_info);
        let struct_def = generate_struct_definition(machine_info, &name_ident, &generics);
        let builder_methods = machine_info.generate_builder_methods();

        quote! {
            #struct_def
            #builder_methods
        }
    } else {
        quote! { compile_error!("Struct not found in machine_map."); }
    }
}

fn parse_generics(machine_info: &MachineInfo) -> Generics {
    let state_enum = &machine_info.get_matching_state_enum();
    let generics_str = machine_info.generics.trim().replace(
        &state_enum.name,
        &format!("S: {} = Uninitialized{}", state_enum.get_trait_name(), &state_enum.name),
    );
    syn::parse_str::<Generics>(&generics_str).expect("Failed to parse generics.")
}

fn generate_struct_definition(
    machine_info: &MachineInfo,
    name_ident: &Ident,
    generics: &Generics,
) -> TokenStream {
    let fields = machine_info.fields.iter().map(|field| {
        let field_ident = format_ident!("{}", field.name);
        let field_ty = syn::parse_str::<syn::Type>(&field.field_type).unwrap();
        quote! { pub #field_ident: #field_ty }
    });

    let derives = &machine_info.derives;

    quote! {
        #[derive(#(#derives),*)]
        pub struct #name_ident #generics {
            #(#fields),*,
            marker: core::marker::PhantomData<S>,
            state_data: S::Data,
        }
    }
}

impl MachineInfo {
    pub fn get_matching_state_enum(&self) -> EnumInfo {
        read_state_enum_map()
            .get(&self.file_path.clone().into())
            .expect("Failed to read state_enum_map.")
            .clone()
    }

    pub fn generate_builder_methods(&self) -> TokenStream {
        let state_enum = self.get_matching_state_enum();
        let fields_map = self
            .fields
            .iter()
            .map(|field| {
                // produce tokens for each field
                let field_ident = format_ident!("{}", field.name);
                let field_ty = syn::parse_str::<syn::Type>(&field.field_type).unwrap();
                quote! { #field_ident: #field_ty }
            }).collect::<Vec<_>>();

        let field_names = self
            .fields
            .iter()
            .map(|field| {
                // produce tokens for each field
                let field_ident = format_ident!("{}", field.name);
                quote! { #field_ident }
            })
            .collect::<Vec<_>>();

        let name_ident = format_ident!("{}", self.name);

        // Generate a builder method for each variant in the state enum.
        let builder_methods = state_enum.variants.iter().map(|variant| {
            let variant_ident = format_ident!("{}", variant.name);
            let variant_builder_ident = format_ident!("{}Builder", variant.name);
            let lowercase_variant_name = format_ident!("{}_builder", variant.name.to_lowercase());
            
            if let Some(ref data_type_str) = variant.data_type {
                // For variants with associated data, parse the type.
                let parsed_data_type = syn::parse_str::<syn::Type>(data_type_str)
                    .expect("Failed to parse state data type");
                    
                quote! {
                    #[statum::bon::bon(crate = ::statum::bon)]
                    impl #name_ident<#variant_ident> {
                        #[builder(state_mod = #lowercase_variant_name, builder_type = #variant_builder_ident)]
                        pub fn new(#(#fields_map),*, state_data: #parsed_data_type) -> #name_ident<#variant_ident> {
                            #name_ident {
                                #(#field_names),*,
                                marker: core::marker::PhantomData,
                                state_data,
                            }
                        }
                    }
                }
            } else {
                // For unit variants, no state_data parameter is needed.
                quote! {
                    #[statum::bon::bon(crate = ::statum::bon)]
                    impl #name_ident<#variant_ident> {
                        #[builder(state_mod = #lowercase_variant_name, builder_type = #variant_builder_ident)]
                        pub fn new(#(#fields_map),*,) -> #name_ident<#variant_ident> {
                            #name_ident {
                                #(#field_names),*,
                                marker: core::marker::PhantomData,
                                state_data: (),
                            }
                        }
                    }
                }
            }
        });

        quote! {
            #(#builder_methods)*
        }
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
