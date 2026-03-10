use macro_registry::analysis::{FileAnalysis, StructEntry, get_file_analysis};
use macro_registry::callsite::{current_module_path, current_source_info};
use macro_registry::registry::{
    RegistryDomain, RegistryKey, RegistryValue, StaticRegistry, ensure_loaded,
};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use std::collections::HashMap;
use std::sync::RwLock;
use syn::{Attribute, GenericParam, Generics, Ident, ItemStruct, LitStr, Visibility};

use crate::{ensure_state_enum_loaded, to_snake_case, EnumInfo, StateModulePath};

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
        self.fields
            .iter()
            .map(|field| format_ident!("{}", field.name))
            .collect::<Vec<_>>()
    }

    pub fn fields_with_types(&self) -> Result<Vec<TokenStream>, syn::Error> {
        let mut fields = Vec::with_capacity(self.fields.len());
        for field in &self.fields {
            let field_ident = format_ident!("{}", field.name);
            let field_ty = syn::parse_str::<syn::Type>(&field.field_type).map_err(|_| {
                syn::Error::new(
                    Span::call_site(),
                    format!(
                        "Failed to parse machine field type `{}` for field `{}`.",
                        field.field_type, field.name
                    ),
                )
            })?;
            fields.push(quote! { #field_ident: #field_ty });
        }
        Ok(fields)
    }
}

impl RegistryValue for MachineInfo {
    fn file_path(&self) -> Option<&str> {
        self.file_path.as_deref()
    }

    fn set_file_path(&mut self, file_path: String) {
        self.file_path = Some(file_path);
    }
}

// Structure to store each field in the struct
#[derive(Debug, Clone)]
pub struct MachineField {
    pub name: String,
    pub vis: String,
    pub field_type: String,
}

// Type-safe wrapper for struct names
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct MachinePath(pub String);

impl AsRef<str> for MachinePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl RegistryKey for MachinePath {
    fn from_module_path(module_path: String) -> Self {
        Self(module_path)
    }
}

// Global storage for all `#[machine]` structs
static MACHINE_MAP: StaticRegistry<MachinePath, MachineInfo> = StaticRegistry::new();

struct MachineRegistryDomain;

impl RegistryDomain for MachineRegistryDomain {
    type Key = MachinePath;
    type Value = MachineInfo;
    type Entry = StructEntry;

    fn entries(analysis: &FileAnalysis) -> &[Self::Entry] {
        &analysis.structs
    }

    fn entry_line(entry: &Self::Entry) -> usize {
        entry.line_number
    }

    fn build_value(entry: &Self::Entry, module_path: &Self::Key) -> Option<Self::Value> {
        MachineInfo::from_item_struct_with_module(&entry.item, module_path)
    }

    fn matches_entry(entry: &Self::Entry) -> bool {
        entry.attrs.iter().any(|attr| attr == "machine")
    }
}

pub fn get_machine_map() -> &'static RwLock<HashMap<MachinePath, MachineInfo>> {
    MACHINE_MAP.map()
}

pub fn get_machine(machine_path: &MachinePath) -> Option<MachineInfo> {
    MACHINE_MAP.get_cloned(machine_path)
}

pub fn ensure_machine_loaded(machine_path: &MachinePath) -> Option<MachineInfo> {
    ensure_loaded::<MachineRegistryDomain>(&MACHINE_MAP, machine_path)
}

pub fn ensure_machine_loaded_by_name(
    machine_path: &MachinePath,
    machine_name: &str,
) -> Option<MachineInfo> {
    if let Some(existing) = get_machine(machine_path)
        && existing.name == machine_name
    {
        return Some(existing);
    }

    if let Some((file_path, _)) = current_source_info()
        && let Some(analysis) = get_file_analysis(&file_path)
    {
        for entry in &analysis.structs {
            if entry.item.ident != machine_name {
                continue;
            }
            if !entry.attrs.iter().any(|attr| attr == "machine") {
                continue;
            }
            if let Some(info) = MachineInfo::from_item_struct_with_module(&entry.item, machine_path) {
                MACHINE_MAP.insert(machine_path.clone(), info.clone());
                return Some(info);
            }
        }
    }

    let loaded = ensure_machine_loaded(machine_path)?;
    (loaded.name == machine_name).then_some(loaded)
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
                    vis: field.vis.to_token_stream().to_string(),
                    field_type: field.ty.to_token_stream().to_string(),
                })
            })
            .collect();

        let module_path = current_module_path();
        let file_path = current_source_info().map(|(path, _)| path);

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
                    vis: field.vis.to_token_stream().to_string(),
                    field_type: field.ty.to_token_stream().to_string(),
                })
            })
            .collect();

        if item.generics.params.is_empty() {
            return None;
        }

        let file_path = current_source_info().map(|(path, _)| path);
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
        match syn::parse_str::<syn::Path>(&self.0) {
            Ok(path) => path.to_tokens(tokens),
            Err(_) => {
                let message = LitStr::new(
                    "Invalid machine module path tokenization.",
                    Span::call_site(),
                );
                tokens.extend(quote! { compile_error!(#message); });
            }
        }
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
    let map_guard = match get_machine_map().read() {
        Ok(guard) => guard,
        Err(_) => {
            return quote! {
                compile_error!("Internal error: machine metadata lock poisoned.");
            };
        }
    };
    let Some(machine_info) = map_guard.get(&machine_info.module_path) else {
        return quote! {
            compile_error!("Internal error: machine metadata not found. Try re-running `cargo check` or ensuring #[machine] is applied in this module.");
        };
    };

    let state_enum = match machine_info.get_matching_state_enum() {
        Ok(enum_info) => enum_info,
        Err(err) => return err,
    };
    let name_ident = format_ident!("{}", machine_info.name);
    let superstate_ident = format_ident!("{}SuperState", machine_info.name);
    let generics = match parse_generics(machine_info, &state_enum) {
        Ok(generics) => generics,
        Err(err) => return err,
    };
    let state_generic_ident = match extract_state_generic_ident(&generics) {
        Ok(ident) => ident,
        Err(err) => return err,
    };
    let struct_def = match generate_struct_definition(
        machine_info,
        &name_ident,
        &generics,
        &state_generic_ident,
    ) {
        Ok(def) => def,
        Err(err) => return err,
    };
    let builder_methods = machine_info.generate_builder_methods(&state_enum);
    let transition_traits = transition_traits(machine_info, &state_enum);
    let superstate =
        match generate_superstate(machine_info, &state_enum, &name_ident, &superstate_ident) {
            Ok(state) => state,
            Err(err) => return err,
        };

    quote! {
        #transition_traits
        #struct_def
        #builder_methods
        #superstate
    }
}

fn parse_generics(machine_info: &MachineInfo, state_enum: &EnumInfo) -> Result<Generics, TokenStream> {
    let mut generics =
        syn::parse_str::<Generics>(&machine_info.generics).map_err(|err| err.to_compile_error())?;

    let Some(first_param) = generics.params.first_mut() else {
        return Err(
            syn::Error::new(
                Span::call_site(),
                "Machine struct must have a state generic as its first type parameter.",
            )
            .to_compile_error(),
        );
    };

    let GenericParam::Type(first_type) = first_param else {
        return Err(
            syn::Error::new(
                Span::call_site(),
                "Machine state generic must be a type parameter.",
            )
            .to_compile_error(),
        );
    };

    let state_trait_ident = state_enum.get_trait_name();
    let has_state_trait_bound = first_type.bounds.iter().any(|bound| {
        matches!(
            bound,
            syn::TypeParamBound::Trait(trait_bound)
            if trait_bound.path.is_ident(&state_trait_ident)
        )
    });
    if !has_state_trait_bound {
        first_type.bounds.push(syn::parse_quote!(#state_trait_ident));
    }

    let default_state_ident = format_ident!("Uninitialized{}", state_enum.name);
    first_type.default = Some(syn::parse_quote!(#default_state_ident));
    first_type.eq_token = Some(syn::Token![=](Span::call_site()));

    Ok(generics)
}

fn extract_state_generic_ident(generics: &Generics) -> Result<Ident, TokenStream> {
    let Some(first_param) = generics.params.first() else {
        return Err(
            syn::Error::new(
                Span::call_site(),
                "Machine struct must have a state generic as its first type parameter.",
            )
            .to_compile_error(),
        );
    };

    if let GenericParam::Type(first_type) = first_param {
        return Ok(first_type.ident.clone());
    }

    Err(
        syn::Error::new(
            Span::call_site(),
            "Machine state generic must be a type parameter.",
        )
        .to_compile_error(),
    )
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

fn generate_superstate(
    machine_info: &MachineInfo,
    state_enum: &EnumInfo,
    machine_ident: &Ident,
    superstate_ident: &Ident,
) -> Result<TokenStream, TokenStream> {
    let superstate_variants = state_enum.variants.iter().map(|variant| {
        let variant_ident = format_ident!("{}", variant.name);
        quote! {
            #variant_ident(#machine_ident<#variant_ident>)
        }
    });

    let vis: Visibility =
        syn::parse_str(&machine_info.vis).map_err(|err| err.to_compile_error())?;

    let is_methods = state_enum.variants.iter().map(|variant| {
        let variant_ident = format_ident!("{}", variant.name);
        let fn_name = format_ident!("is_{}", to_snake_case(&variant.name));
        quote! {
            pub fn #fn_name(&self) -> bool {
                matches!(self, #superstate_ident::#variant_ident(_))
            }
        }
    });

    let module_ident = format_ident!("{}", to_snake_case(&machine_info.name));

    Ok(quote! {
        #vis enum #superstate_ident {
            #(#superstate_variants),*
        }

        impl #superstate_ident {
            #(#is_methods)*
        }

        #vis mod #module_ident {
            pub type State = super::#superstate_ident;
        }
    })
}

fn generate_struct_definition(
    machine_info: &MachineInfo,
    name_ident: &Ident,
    generics: &Generics,
    state_generic_ident: &Ident,
) -> Result<TokenStream, TokenStream> {
    let mut field_tokens = Vec::with_capacity(machine_info.fields.len());
    for field in &machine_info.fields {
        let field_ident = format_ident!("{}", field.name);
        let field_vis =
            syn::parse_str::<Visibility>(&field.vis).map_err(|err| err.to_compile_error())?;
        let field_ty = syn::parse_str::<syn::Type>(&field.field_type)
            .map_err(|err| err.to_compile_error())?;
        field_tokens.push(quote! { #field_vis #field_ident: #field_ty });
    }

    let derives = if machine_info.derives.is_empty() {
        quote! {}
    } else {
        let mut derive_tokens = Vec::with_capacity(machine_info.derives.len());
        for derive in &machine_info.derives {
            let parsed = syn::parse_str::<syn::Path>(derive).map_err(|err| err.to_compile_error())?;
            derive_tokens.push(parsed);
        }
        quote! {
            #[derive(#(#derive_tokens),*)]
        }
    };

    let vis: Visibility =
        syn::parse_str(&machine_info.vis).map_err(|err| err.to_compile_error())?;

    Ok(quote! {
        #derives
        #vis struct #name_ident #generics {
            marker: core::marker::PhantomData<#state_generic_ident>,
            pub state_data: #state_generic_ident::Data,
            #( #field_tokens ),*
        }
    })
}

impl MachineInfo {
    pub(crate) fn expected_state_name(&self) -> Option<String> {
        let generics = syn::parse_str::<Generics>(&self.generics).ok()?;
        let first_param = generics.params.first()?;
        if let syn::GenericParam::Type(ty) = first_param {
            Some(ty.ident.to_string())
        } else {
            None
        }
    }

    pub fn get_matching_state_enum(&self) -> Result<EnumInfo, TokenStream> {
        let state_path: StateModulePath = self.module_path.clone().into();
        let Some(state_enum) = ensure_state_enum_loaded(&state_path) else {
            return Err(missing_state_enum_error(self));
        };
        Ok(state_enum)
    }

    pub fn generate_builder_methods(&self, state_enum: &EnumInfo) -> TokenStream {
        let mut fields_map = Vec::with_capacity(self.fields.len());
        for field in &self.fields {
            let field_ident = format_ident!("{}", field.name);
            let field_ty = match syn::parse_str::<syn::Type>(&field.field_type) {
                Ok(ty) => ty,
                Err(err) => return err.to_compile_error(),
            };
            fields_map.push(quote! { #field_ident: #field_ty });
        }

        let field_names = self
            .fields
            .iter()
            .map(|field| {
                let field_ident = format_ident!("{}", field.name);
                quote! { #field_ident }
            })
            .collect::<Vec<_>>();
        let mut field_types = Vec::with_capacity(self.fields.len());
        for field in &self.fields {
            let ty = match syn::parse_str::<syn::Type>(&field.field_type) {
                Ok(ty) => ty,
                Err(err) => return err.to_compile_error(),
            };
            field_types.push(ty);
        }

        let name_ident = format_ident!("{}", self.name);

        let use_ra_shim = is_rust_analyzer();
        // Generate a builder method for each variant in the state enum.
        let builder_methods = state_enum.variants.iter().map(|variant| {
            let variant_ident = format_ident!("{}", variant.name);
            let variant_builder_ident = format_ident!("{}Builder", variant.name);
            let lowercase_variant_name = format_ident!("{}_builder", variant.name.to_lowercase());

            if let Some(ref data_type_str) = variant.data_type {
                // For variants with associated data, parse the type.
                let parsed_data_type = match syn::parse_str::<syn::Type>(data_type_str) {
                    Ok(ty) => ty,
                    Err(err) => return err.to_compile_error(),
                };

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
                                panic!("statum rust-analyzer shim: builder values are not constructed at runtime")
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
                                panic!("statum rust-analyzer shim: builder values are not constructed at runtime")
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

    if item.generics.params.len() > 1 {
        let message = format!(
            "Error: #[machine] currently supports exactly one generic type parameter: `{state_name}`.\n\n\
Fix: Remove additional generics from `{machine_name}` and keep stateful context in fields.\n\n\
Expected:\n\
pub struct {machine_name}<{state_name}> {{ ... }}"
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
    MACHINE_MAP.insert(machine_info.module_path.clone(), machine_info.clone());
}
