use module_path_extractor::get_pseudo_module_path;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use syn::{Attribute, Generics, Ident, ItemStruct, Visibility};

use crate::{read_state_enum_map, EnumInfo, StateModulePath};

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
}

impl MachineInfo{
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

// Extract derives from `#[derive(Debug, Clone, ...)]`

pub fn extract_derive(attr: &Attribute) -> Option<Vec<String>> {
    if !attr.path().is_ident("derive") {
        return None;
    }
    attr.meta.require_list().ok()?.parse_args_with(
        syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
    ).ok()
    .map(|punctuated| {
        punctuated.iter().map(|p| p.to_token_stream().to_string()).collect()
    })
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


        let module_path = get_pseudo_module_path();

        Ok(Self {
            name: item.ident.to_string(),
            vis: item.vis.to_token_stream().to_string(),
            derives: item.attrs.iter().filter_map(extract_derive).flatten().collect(),
            fields,
            module_path: module_path.into(),
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
    if let Some(machine_info) = get_machine_map().read().unwrap().get(&machine_info.module_path) {
        let name_ident = format_ident!("{}", machine_info.name);
        let generics = parse_generics(machine_info);
        let struct_def = generate_struct_definition(machine_info, &name_ident, &generics);
        let builder_methods = machine_info.generate_builder_methods();
        let transition_traits = transition_traits(machine_info);

        quote! {
            #transition_traits
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

fn transition_traits(machine_info: &MachineInfo) -> TokenStream {
    let state_enum = machine_info.get_matching_state_enum();
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
        let derive_tokens = machine_info.derives.iter().map(|d| syn::parse_str::<syn::Path>(d).unwrap());
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
    pub fn get_matching_state_enum(&self) -> EnumInfo {
        read_state_enum_map()
            .get(&self.module_path.clone().into())
            .expect("Failed to read state_enum_map.")
            .clone()
    }

    pub fn generate_builder_methods(&self) -> TokenStream {
        let state_enum = self.get_matching_state_enum();

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

                quote! {
                    #[statum::bon::bon(crate = ::statum::bon)]
                    impl #name_ident<#variant_ident> {
                        #[builder(state_mod = #lowercase_variant_name, builder_type = #variant_builder_ident)]
                        #constructor_signature {
                            #struct_initialization
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
    let matching_state_enum = machine_info.get_matching_state_enum();

    let machine_derives: Vec<String> = machine_info.derives.clone();
    let state_derives: Vec<String> = matching_state_enum.derives.clone();

    // Find which derives are missing from the #[state] enum
    let missing_derives: Vec<String> = machine_derives
        .iter()
        .filter(|derive| !state_derives.contains(derive))
        .cloned()
        .collect();

    if !missing_derives.is_empty() {
        let missing_list = missing_derives.join(", ");
        return Some(quote! {
            compile_error!(concat!(
                "The #[state] enum is missing required derives: ",
                #missing_list,
                "\nFix: Add the missing derives to your #[state] enum.\n",
                "Example:\n\n",
                "#[state]\n",
                "#[derive(", #missing_list, ")]\n",
                "pub enum State { Off, On }"
            ));
        });
    }

    let state_name = matching_state_enum.name.clone();
    let machine_name = machine_info.name.clone();

    // Ensure it's applied to a struct
    if !matches!(item, ItemStruct { .. }) {
        return Some(quote! {
            compile_error!(concat!(
                "Error: #[machine] must be applied to a struct.\n\n",
                "Fix: Apply #[machine] to a struct instead of another type.\n\n",
                "Example:\n\n",
                "#[state]\n",
                "pub enum ", #state_name, " { ... }\n\n",
                "#[machine]\n",
                "pub struct ", #machine_name, "<", #state_name, "> { ... }"
            ));
        });
    }

    // Ensure the struct has at least one generic type parameter matching the #[state] enum
    let first_generic_param = item.generics.params.first().map(|param| param.to_token_stream().to_string());
    if first_generic_param.as_deref() != Some(&state_name) {
        return Some(quote! {
            compile_error!(concat!(
                "Error: #[machine] structs must have a generic type parameter that matches the #[state] enum.\n\n",
                "Fix: Change the generic type parameter of `", #machine_name, "` to match `", #state_name, "`.\n\n",
                "Expected:\n",
                "pub struct ", #machine_name, "<", #state_name, "> { ... }\n\n",
                "Found:\n",
                "pub struct ", #machine_name, "<", first_generic_param.unwrap_or("<missing>").as_str(), "> { ... }"
            ));
        });
    }

    None
}

pub fn store_machine_struct(machine_info: &MachineInfo) {
    let mut map = get_machine_map().write().unwrap();
    map.insert(machine_info.module_path.clone(), machine_info.clone());
}
