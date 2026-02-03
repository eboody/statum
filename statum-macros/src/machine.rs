use module_path_extractor::{find_module_path, get_pseudo_module_path, get_source_info};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use std::collections::HashMap;
use std::fs;
use std::sync::{OnceLock, RwLock};
use syn::{Attribute, Generics, Ident, ItemStruct, LitStr, Visibility};

use crate::{ensure_state_enum_loaded, EnumInfo, StateModulePath};

impl<T: ToString> From<T> for MachinePath {
    fn from(value: T) -> Self {
        Self(value.to_string())
    }
}

// Convert MachinePath to StatePath
impl From<MachinePath> for StateModulePath {
    fn from(machine: MachinePath) -> Self {
        StateModulePath(machine.0)
    }
}

// Structure to store metadata about a struct
#[derive(Debug, Clone)]
pub struct MachineInfo {
    pub name: String,
    pub vis: String,
    pub derives: Vec<String>,
    pub fields: Vec<MachineField>,
    pub module_path: MachinePath,
    pub generics: String,
    pub file_path: Option<String>,
}

impl MachineInfo {
    pub fn field_names(&self) -> Vec<Ident> {
        let field_names = self
            .fields
            .iter()
            .map(|field| format_ident!("{}", field.name))
            .collect::<Vec<_>>();
        field_names
    }

    pub fn fields_with_types(&self) -> Vec<TokenStream> {
        let fields_map = self
            .fields
            .iter()
            .map(|field| {
                let field_ident = format_ident!("{}", field.name);
                let field_ty = syn::parse_str::<syn::Type>(&field.field_type).unwrap();
                quote! { #field_ident: #field_ty }
            })
            .collect::<Vec<_>>();
        fields_map
    }
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

pub fn ensure_machine_loaded(machine_path: &MachinePath) -> Option<MachineInfo> {
    let source_info = get_source_info()?;
    let file_path = source_info.0;
    if let Some(info) = get_machine_map().read().ok()?.get(machine_path).cloned() {
        if info.file_path.as_deref() == Some(file_path.as_str()) {
            return Some(info);
        }
    }

    let contents = fs::read_to_string(&file_path).ok()?;
    let parsed = syn::parse_file(&contents).ok()?;
    let allow_any_module = machine_path.0 == "unknown";

    let mut found: Option<MachineInfo> = None;
    for item in parsed.items {
        let struct_item = match item {
            syn::Item::Struct(item_struct) => item_struct,
            _ => continue,
        };

        if !struct_item
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("machine"))
        {
            continue;
        }

        let struct_name = struct_item.ident.to_string();
        let line_number = find_item_line(&contents, &struct_name)?;
        let module_path = find_module_path(&file_path, line_number)?;
        if !allow_any_module && &module_path != &machine_path.0 {
            continue;
        }

        let mut machine_info = MachineInfo::from_item_struct_with_module(&struct_item, machine_path)?;
        machine_info.file_path = Some(file_path.clone());
        found = Some(machine_info);
        break;
    }

    if let Some(machine_info) = found.clone() {
        store_machine_struct(&machine_info);
    }

    found
}

fn find_item_line(contents: &str, item_name: &str) -> Option<usize> {
    for (idx, line) in contents.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("struct ") || trimmed.starts_with("pub struct ") {
            if trimmed.contains(&format!("struct {}", item_name)) {
                return Some(idx + 1);
            }
        }
    }
    None
}

// Extract derives from `#[derive(Debug, Clone, ...)]`

pub fn extract_derive(attr: &Attribute) -> Option<Vec<String>> {
    if !attr.path().is_ident("derive") {
        return None;
    }
    attr.meta
        .require_list()
        .ok()?
        .parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated)
        .ok()
        .map(|punctuated| {
            punctuated
                .iter()
                .map(|p| p.to_token_stream().to_string())
                .collect()
        })
}

// Extracts machine struct information
impl MachineInfo {
    pub fn from_item_struct(item: &ItemStruct) -> Self {
        let fields = item
            .fields
            .iter()
            .filter_map(|field| {
                field.ident.as_ref().map(|ident| MachineField {
                    name: ident.to_string(),
                    field_type: field.ty.to_token_stream().to_string(),
                })
            })
            .collect();

        let module_path = get_pseudo_module_path();
        let file_path = get_source_info().map(|(path, _)| path);

        Self {
            name: item.ident.to_string(),
            vis: item.vis.to_token_stream().to_string(),
            derives: item
                .attrs
                .iter()
                .filter_map(extract_derive)
                .flatten()
                .collect(),
            fields,
            module_path: module_path.into(),
            generics: item.generics.to_token_stream().to_string(),
            file_path,
        }
    }

    pub fn from_item_struct_with_module(
        item: &ItemStruct,
        module_path: &MachinePath,
    ) -> Option<Self> {
        let fields = item
            .fields
            .iter()
            .filter_map(|field| {
                field.ident.as_ref().map(|ident| MachineField {
                    name: ident.to_string(),
                    field_type: field.ty.to_token_stream().to_string(),
                })
            })
            .collect();

        if item.generics.params.is_empty() {
            return None;
        }

        let file_path = get_source_info().map(|(path, _)| path);
        Some(Self {
            name: item.ident.to_string(),
            vis: item.vis.to_token_stream().to_string(),
            derives: item
                .attrs
                .iter()
                .filter_map(extract_derive)
                .flatten()
                .collect(),
            fields,
            module_path: module_path.clone(),
            generics: item.generics.to_token_stream().to_string(),
            file_path,
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

//impl MachineInfo {
//    pub fn fields_to_token_stream(&self) -> TokenStream {
//        let fields = self.fields.iter().map(|field| {
//            let field_ident = format_ident!("{}", field.name);
//            quote! { #field_ident: self.#field_ident, }
//        });
//
//        quote! {
//            #(#fields)*
//        }
//    }
//}

// Generates struct-based metadata implementations
pub fn generate_machine_impls(machine_info: &MachineInfo) -> proc_macro2::TokenStream {
    if let Some(machine_info) = get_machine_map()
        .read()
        .unwrap()
        .get(&machine_info.module_path)
    {
        let state_enum = match machine_info.get_matching_state_enum() {
            Ok(enum_info) => enum_info,
            Err(err) => return err,
        };
        let name_ident = format_ident!("{}", machine_info.name);
        let generics = parse_generics(machine_info, &state_enum);
        let struct_def = generate_struct_definition(machine_info, &name_ident, &generics);
        let builder_methods = machine_info.generate_builder_methods(&state_enum);
        let transition_traits = transition_traits(machine_info, &state_enum);

        quote! {
            #transition_traits
            #struct_def
            #builder_methods
        }
    } else {
        quote! { compile_error!("Internal error: machine metadata not found. Try re-running `cargo check` or ensuring #[machine] is applied in this module."); }
    }
}

fn parse_generics(machine_info: &MachineInfo, state_enum: &EnumInfo) -> Generics {
    let generics_str = machine_info.generics.trim().replace(
        &state_enum.name,
        &format!(
            "S: {} = Uninitialized{}",
            state_enum.get_trait_name(),
            &state_enum.name
        ),
    );
    syn::parse_str::<Generics>(&generics_str).expect("Failed to parse generics.")
}

fn transition_traits(machine_info: &MachineInfo, state_enum: &EnumInfo) -> TokenStream {
    let trait_name = state_enum.get_trait_name();
    let machine_name = format_ident!("{}", machine_info.name);
    quote! {
        pub trait TransitionTo<N: #trait_name> {
            fn transition(self) -> #machine_name<N>;
        }

        pub trait TransitionWith<T> {
            type NextState: #trait_name;
            fn transition_with(self, data: T) -> #machine_name<Self::NextState>;
        }
    }
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

    let derives = if machine_info.derives.is_empty() {
        quote! {}
    } else {
        let derive_tokens = machine_info
            .derives
            .iter()
            .map(|d| syn::parse_str::<syn::Path>(d).unwrap());
        quote! {
            #[derive(#(#derive_tokens),*)]
        }
    };

    let vis: Visibility = syn::parse_str(&machine_info.vis).expect("Failed to parse visibility.");

    quote! {
        #derives
        #vis struct #name_ident #generics {
            marker: core::marker::PhantomData<S>,
            pub state_data: S::Data,
            #( #fields ),*
        }
    }
}

impl MachineInfo {
    pub fn get_matching_state_enum(&self) -> Result<EnumInfo, TokenStream> {
        ensure_state_enum_loaded(&self.module_path.clone().into())
            .ok_or_else(|| missing_state_enum_error(self))
    }

    pub fn generate_builder_methods(&self, state_enum: &EnumInfo) -> TokenStream {
        let fields_map = self
            .fields
            .iter()
            .map(|field| {
                let field_ident = format_ident!("{}", field.name);
                let field_ty = syn::parse_str::<syn::Type>(&field.field_type).unwrap();
                quote! { #field_ident: #field_ty }
            })
            .collect::<Vec<_>>();

        let field_names = self
            .fields
            .iter()
            .map(|field| {
                let field_ident = format_ident!("{}", field.name);
                quote! { #field_ident }
            })
            .collect::<Vec<_>>();
        let field_types = self
            .fields
            .iter()
            .map(|field| syn::parse_str::<syn::Type>(&field.field_type).unwrap())
            .collect::<Vec<_>>();

        let name_ident = format_ident!("{}", self.name);

        let use_ra_shim = is_rust_analyzer();
        // Generate a builder method for each variant in the state enum.
        let builder_methods = state_enum.variants.iter().map(|variant| {
            let variant_ident = format_ident!("{}", variant.name);
            let variant_builder_ident = format_ident!("{}Builder", variant.name);
            let lowercase_variant_name = format_ident!("{}_builder", variant.name.to_lowercase());

            if let Some(ref data_type_str) = variant.data_type {
                // For variants with associated data, parse the type.
                let parsed_data_type = syn::parse_str::<syn::Type>(data_type_str)
                    .expect("Failed to parse state data type");

                let struct_initialization = if self.fields.is_empty() {
                    quote! {
                        #name_ident {
                            marker: core::marker::PhantomData,
                            state_data,
                        }
                    }
                } else {
                    quote! {
                        #name_ident {
                            marker: core::marker::PhantomData,
                            state_data,
                            #(#field_names,)*
                        }
                    }
                };

                let constructor_signature = if self.fields.is_empty() {
                    quote! {
                        pub fn new(state_data: #parsed_data_type) -> #name_ident<#variant_ident>
                    }
                } else {
                    quote! {
                        pub fn new(#(#fields_map,)* state_data: #parsed_data_type) -> #name_ident<#variant_ident>
                    }
                };

                if use_ra_shim {
                    quote! {
                        pub struct #variant_builder_ident;

                        impl #variant_builder_ident {
                            pub fn state_data(self, _data: #parsed_data_type) -> Self {
                                self
                            }

                            #(pub fn #field_names(self, _value: #field_types) -> Self { self })*

                            pub fn build(self) -> #name_ident<#variant_ident> {
                                unsafe { core::mem::MaybeUninit::uninit().assume_init() }
                            }
                        }

                        impl #name_ident<#variant_ident> {
                            pub fn builder() -> #variant_builder_ident {
                                #variant_builder_ident
                            }
                        }
                    }
                } else {
                    quote! {
                        #[statum::bon::bon(crate = ::statum::bon)]
                        impl #name_ident<#variant_ident> {
                            #[builder(state_mod = #lowercase_variant_name, builder_type = #variant_builder_ident)]
                            #constructor_signature {
                                #struct_initialization
                            }
                        }
                    }
                }
            } else {
                // For unit variants (no state data)
                let struct_initialization = if self.fields.is_empty() {
                    quote! {
                        #name_ident {
                            marker: core::marker::PhantomData,
                            state_data: (),
                        }
                    }
                } else {
                    quote! {
                        #name_ident {
                            marker: core::marker::PhantomData,
                            state_data: (),
                            #(#field_names),*
                        }
                    }
                };

                let constructor_signature = if self.fields.is_empty() {
                    quote! {
                        pub fn new() -> #name_ident<#variant_ident>
                    }
                } else {
                    quote! {
                        pub fn new(#(#fields_map),*) -> #name_ident<#variant_ident>
                    }
                };

                if use_ra_shim {
                    quote! {
                        pub struct #variant_builder_ident;

                        impl #variant_builder_ident {
                            #(pub fn #field_names(self, _value: #field_types) -> Self { self })*

                            pub fn build(self) -> #name_ident<#variant_ident> {
                                unsafe { core::mem::MaybeUninit::uninit().assume_init() }
                            }
                        }

                        impl #name_ident<#variant_ident> {
                            pub fn builder() -> #variant_builder_ident {
                                #variant_builder_ident
                            }
                        }
                    }
                } else {
                    quote! {
                        #[statum::bon::bon(crate = ::statum::bon)]
                        impl #name_ident<#variant_ident> {
                            #[builder(state_mod = #lowercase_variant_name, builder_type = #variant_builder_ident)]
                            #constructor_signature {
                                #struct_initialization
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

fn missing_state_enum_error(machine_info: &MachineInfo) -> TokenStream {
    if is_rust_analyzer() {
        return TokenStream::new();
    }
    let message = format!(
        "Failed to resolve the #[state] enum for machine `{}`. \
This can happen if proc-macro analysis runs before the enum is cached. \
Try reopening the file or running `cargo check`. If it persists, ensure the #[state] enum is in the same module.",
        machine_info.name
    );
    let message = LitStr::new(&message, Span::call_site());
    quote! { compile_error!(#message); }
}

pub fn validate_machine_struct(
    item: &ItemStruct,
    machine_info: &MachineInfo,
) -> Option<TokenStream> {
    let matching_state_enum = match machine_info.get_matching_state_enum() {
        Ok(enum_info) => enum_info,
        Err(err) => return Some(err),
    };

    let machine_derives: Vec<String> = machine_info.derives.clone();
    let state_derives: Vec<String> = matching_state_enum.derives.clone();
    let state_name = matching_state_enum.name.clone();

    // Find which derives are missing from the #[state] enum
    let missing_derives: Vec<String> = machine_derives
        .iter()
        .filter(|derive| !state_derives.contains(derive))
        .cloned()
        .collect();

    if !missing_derives.is_empty() && !is_rust_analyzer() {
        let missing_list = missing_derives.join(", ");
        let message = format!(
            "The #[state] enum `{state_name}` is missing required derives: {missing_list}\n\
Fix: Add the missing derives to your #[state] enum.\n\
Example:\n\n\
#[state]\n\
#[derive({missing_list})]\n\
pub enum State {{ Off, On }}",
        );
        let message = LitStr::new(&message, Span::call_site());
        return Some(quote! {
            compile_error!(#message);
        });
    }

    let machine_name = machine_info.name.clone();

    // Ensure it's applied to a struct
    if !matches!(item, ItemStruct { .. }) {
        let message = format!(
            "Error: #[machine] must be applied to a struct.\n\n\
Fix: Apply #[machine] to a struct instead of another type.\n\n\
Example:\n\n\
#[state]\n\
pub enum {state_name} {{ ... }}\n\n\
#[machine]\n\
pub struct {machine_name}<{state_name}> {{ ... }}"
        );
        let message = LitStr::new(&message, Span::call_site());
        return Some(quote! {
            compile_error!(#message);
        });
    }

    // Ensure the struct has at least one generic type parameter matching the #[state] enum
    let first_generic_param = item
        .generics
        .params
        .first()
        .map(|param| param.to_token_stream().to_string());
    if first_generic_param.as_deref() != Some(&state_name) {
        let found = first_generic_param.unwrap_or_else(|| "<missing>".to_string());
        let message = format!(
            "Error: #[machine] structs must have a generic type parameter that matches the #[state] enum.\n\n\
Fix: Change the generic type parameter of `{machine_name}` to match `{state_name}`.\n\n\
Expected:\n\
pub struct {machine_name}<{state_name}> {{ ... }}\n\n\
Found:\n\
pub struct {machine_name}<{found}> {{ ... }}"
        );
        let message = LitStr::new(&message, Span::call_site());
        return Some(quote! {
            compile_error!(#message);
        });
    }

    None
}

fn is_rust_analyzer() -> bool {
    std::env::var("RUST_ANALYZER_INTERNALS").is_ok()
}

pub fn store_machine_struct(machine_info: &MachineInfo) {
    let mut map = get_machine_map().write().unwrap();
    map.insert(machine_info.module_path.clone(), machine_info.clone());
}
