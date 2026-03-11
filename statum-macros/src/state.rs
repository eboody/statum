use macro_registry::analysis::{EnumEntry, FileAnalysis, get_file_analysis};
use macro_registry::callsite::{current_module_path, current_source_info};
use macro_registry::registry::{
    RegistryDomain, RegistryKey, RegistryValue, StaticRegistry, ensure_loaded,
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{Attribute, Fields, Ident, ItemEnum, Path, Type, Visibility};

// Structure to hold extracted enum data
#[derive(Clone)]
#[allow(unused)]
pub struct EnumInfo {
    pub derives: Vec<String>,
    pub vis: String,
    pub name: String,
    pub variants: Vec<VariantInfo>,
    pub generics: String,
    pub module_path: StateModulePath,
    pub file_path: Option<String>,
}

impl EnumInfo {
    pub fn get_variant_from_name(&self, variant_name: &str) -> Option<&VariantInfo> {
        self.variants
            .iter()
            .find(|v| v.name == variant_name || to_snake_case(&v.name) == variant_name)
    }
}

impl RegistryValue for EnumInfo {
    fn file_path(&self) -> Option<&str> {
        self.file_path.as_deref()
    }

    fn set_file_path(&mut self, file_path: String) {
        self.file_path = Some(file_path);
    }
}

#[derive(Clone)]
pub struct VariantInfo {
    pub name: String,
    pub data_type: Option<String>,
}

pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && c.is_uppercase() {
            result.push('_');
        }
        for lowered in c.to_lowercase() {
            result.push(lowered);
        }
    }
    result
}

// Type-safe wrapper around an enum name
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct StateModulePath(pub String);

impl AsRef<str> for StateModulePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl RegistryKey for StateModulePath {
    fn from_module_path(module_path: String) -> Self {
        Self(module_path)
    }
}

impl EnumInfo {
    pub fn get_trait_name(&self) -> Ident {
        format_ident!("{}Trait", self.name)
    }

    pub(crate) fn parse(&self) -> Result<ParsedEnumInfo, TokenStream> {
        let vis = syn::parse_str::<Visibility>(&self.vis).map_err(|err| err.to_compile_error())?;
        let mut derives = Vec::with_capacity(self.derives.len());
        for derive in &self.derives {
            derives.push(syn::parse_str::<Path>(derive).map_err(|err| err.to_compile_error())?);
        }

        let mut variants = Vec::with_capacity(self.variants.len());
        for variant in &self.variants {
            variants.push(ParsedVariantInfo {
                name: variant.name.clone(),
                data_type: variant.parse_data_type()?,
            });
        }

        Ok(ParsedEnumInfo {
            vis,
            derives,
            variants,
        })
    }
}

impl VariantInfo {
    pub(crate) fn parse_data_type(&self) -> Result<Option<Type>, TokenStream> {
        self.data_type
            .as_ref()
            .map(|data_type| syn::parse_str::<Type>(data_type).map_err(|err| err.to_compile_error()))
            .transpose()
    }
}

pub(crate) struct ParsedEnumInfo {
    pub(crate) vis: Visibility,
    pub(crate) derives: Vec<Path>,
    pub(crate) variants: Vec<ParsedVariantInfo>,
}

pub(crate) struct ParsedVariantInfo {
    pub(crate) name: String,
    pub(crate) data_type: Option<Type>,
}

/// Convert `StateEnumName` into a `TokenStream` (for procedural macros)
impl ToTokens for StateModulePath {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match syn::parse_str::<syn::Path>(&self.0) {
            Ok(path) => path.to_tokens(tokens),
            Err(_) => {
                let message = syn::LitStr::new(
                    "Invalid state module path tokenization.",
                    proc_macro2::Span::call_site(),
                );
                tokens.extend(quote! { compile_error!(#message); });
            }
        }
    }
}

/// Convert from `&str` to `StateEnumName`
impl From<&str> for StateModulePath {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Convert from `String` to `StateEnumName`
impl From<String> for StateModulePath {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Convert from `Ident` (Rust identifiers) to `StateEnumName`
impl From<Ident> for StateModulePath {
    fn from(ident: Ident) -> Self {
        Self(ident.to_string())
    }
}

/// Convert from `&Ident` to `StateEnumName`
impl From<&Ident> for StateModulePath {
    fn from(ident: &Ident) -> Self {
        Self(ident.to_string())
    }
}

/// Convert from `TokenStream` to `StateEnumName`
impl From<TokenStream> for StateModulePath {
    fn from(token_stream: TokenStream) -> Self {
        Self(token_stream.to_string())
    }
}

/// Convert `StateEnumName` into a `TokenStream`
impl From<StateModulePath> for TokenStream {
    fn from(state: StateModulePath) -> Self {
        match syn::parse_str::<syn::Path>(&state.0) {
            Ok(path) => quote! { #path },
            Err(err) => err.to_compile_error(),
        }
    }
}

/// Convert `EnumInfo` into a `TokenStream`
impl ToTokens for EnumInfo {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = format_ident!("{}", &self.name);
        let parsed = match self.parse() {
            Ok(parsed) => parsed,
            Err(err) => {
                tokens.extend(err);
                return;
            }
        };
        let vis = parsed.vis;

        let mut variants = Vec::with_capacity(parsed.variants.len());
        for variant in parsed.variants {
            let var_name = syn::Ident::new(&variant.name, proc_macro2::Span::call_site());
            let variant_tokens = match &variant.data_type {
                Some(ty) => quote! { #var_name(#ty) },
                None => quote! { #var_name },
            };
            variants.push(variant_tokens);
        }

        let expanded = quote! {
            #vis enum #name {
                #(#variants),*
            }
        };

        tokens.extend(expanded);
    }
}

// Global storage for `#[state]` enums

static STATE_ENUMS: StaticRegistry<StateModulePath, EnumInfo> = StaticRegistry::new();

struct StateRegistryDomain;

impl RegistryDomain for StateRegistryDomain {
    type Key = StateModulePath;
    type Value = EnumInfo;
    type Entry = EnumEntry;

    fn entries(analysis: &FileAnalysis) -> &[Self::Entry] {
        &analysis.enums
    }

    fn entry_line(entry: &Self::Entry) -> usize {
        entry.line_number
    }

    fn build_value(entry: &Self::Entry, module_path: &Self::Key) -> Option<Self::Value> {
        EnumInfo::from_item_enum_with_module(&entry.item, module_path.clone()).ok()
    }

    fn matches_entry(entry: &Self::Entry) -> bool {
        entry.attrs.iter().any(|attr| attr == "state")
    }
}

pub fn get_state_enum(enum_path: &StateModulePath) -> Option<EnumInfo> {
    STATE_ENUMS.get_cloned(enum_path)
}

pub fn ensure_state_enum_loaded(enum_path: &StateModulePath) -> Option<EnumInfo> {
    ensure_loaded::<StateRegistryDomain>(&STATE_ENUMS, enum_path)
}

pub fn ensure_state_enum_loaded_by_name(
    enum_path: &StateModulePath,
    enum_name: &str,
) -> Option<EnumInfo> {
    if let Some(existing) = get_state_enum(enum_path)
        && existing.name == enum_name
    {
        return Some(existing);
    }

    if let Some((file_path, _)) = current_source_info()
        && let Some(analysis) = get_file_analysis(&file_path)
    {
        for entry in &analysis.enums {
            if entry.item.ident != enum_name {
                continue;
            }
            if !entry.attrs.iter().any(|attr| attr == "state") {
                continue;
            }
            if let Ok(info) = EnumInfo::from_item_enum_with_module(&entry.item, enum_path.clone()) {
                STATE_ENUMS.insert(enum_path.clone(), info.clone());
                return Some(info);
            }
        }
    }

    let loaded = ensure_state_enum_loaded(enum_path)?;
    (loaded.name == enum_name).then_some(loaded)
}
/// Extracts `#[derive(...)]` attributes from an enum
pub fn extract_derive(attr: &Attribute) -> Option<Vec<String>> {
    if attr.path().is_ident("derive") && let Ok(meta) = attr.meta.require_list() {
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
    None
}

impl EnumInfo {
    pub fn from_item_enum(item: &ItemEnum) -> syn::Result<Self> {
        let module_path = current_module_path();
        let file_path = current_source_info().map(|(path, _)| path);
        Self::from_item_enum_with_module_and_file(item, module_path.into(), file_path)
    }

    pub fn from_item_enum_with_module(
        item: &ItemEnum,
        module_path: StateModulePath,
    ) -> syn::Result<Self> {
        let file_path = current_source_info().map(|(path, _)| path);
        Self::from_item_enum_with_module_and_file(item, module_path, file_path)
    }

    fn from_item_enum_with_module_and_file(
        item: &ItemEnum,
        module_path: StateModulePath,
        file_path: Option<String>,
    ) -> syn::Result<Self> {
        let name = item.ident.to_string();
        let vis = item.vis.to_token_stream().to_string();
        // 1.0 policy: generics on `#[state]` enums are intentionally unsupported.
        // `validate_state_enum` emits a compile error when generics are present.
        let generics = item.generics.clone().to_token_stream().to_string();

        let derives = item
            .attrs
            .iter()
            .filter_map(extract_derive)
            .flatten()
            .collect();

        let mut variants = Vec::new();
        for variant in &item.variants {
            let name = variant.ident.to_string();
            let data_type = match &variant.fields {
                Fields::Unnamed(fields) if fields.unnamed.len() == 1 => match fields.unnamed.first() {
                    Some(first) => Some(first.ty.to_token_stream().to_string()),
                    None => {
                        return Err(syn::Error::new_spanned(
                            variant,
                            format!(
                                "Invalid variant `{}` in #[state] enum. \
                                 Variants must be unit or single-field tuple variants.",
                                name
                            ),
                        ));
                    }
                },
                Fields::Unit => None, // ✅ Unit variant is allowed
                _ => {
                    return Err(syn::Error::new_spanned(
                        variant,
                        format!(
                            "Invalid variant `{}` in #[state] enum. \
                             Variants must be unit or single-field tuple variants.",
                            name
                        ),
                    ));
                }
            };

            variants.push(VariantInfo { name, data_type });
        }

        if variants.is_empty() {
            return Err(syn::Error::new_spanned(
                item,
                "Error: #[state] enums must have at least one variant.",
            ));
        }

        Ok(Self {
            derives,
            vis,
            name,
            variants,
            generics,
            module_path,
            file_path,
        })
    }
}

pub fn generate_state_impls(enum_path: &StateModulePath) -> proc_macro2::TokenStream {
    let Some(enum_info) = get_state_enum(enum_path) else {
        return quote! {
            compile_error!("Internal error: state metadata not found. Ensure #[state] is applied in this module.");
        };
    };

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
    // Generate one struct and implementation per variant
    for variant in parsed_enum.variants {
        let variant_name = format_ident!("{}", variant.name);
        let variant_derives = if derive_tokens.is_empty() {
            quote! {}
        } else {
            quote! { #[derive(#(#derive_tokens),*)] }
        };

        let tokens = match &variant.data_type {
            // Handle tuple variants (state has associated data)
            Some(field_type) => {
                let field_ty = field_type.clone();
                quote! {
                    #variant_derives
                    #vis struct #variant_name (pub #field_ty);

                    impl #state_trait_ident for #variant_name {
                        type Data = #field_ty;
                    }

                    impl statum::StateMarker for #variant_name {
                        type Data = #field_ty;
                    }

                    impl statum::DataState for #variant_name {}
                }
            }
            // Handle unit variants (state has no associated data)
            None => {
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

    // Generate the trait definition and include all variant structs
    quote! {
        #state_trait

        #(#variant_structs)*

        #uninitialized_state
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
    STATE_ENUMS.insert(enum_info.module_path.clone(), enum_info.clone());
}

#[cfg(test)]
mod tests {
    use quote::ToTokens;
    use syn::parse_quote;

    use super::{EnumInfo, StateModulePath};

    #[test]
    fn parse_round_trips_variant_payloads() {
        let item: syn::ItemEnum = parse_quote! {
            #[derive(Clone)]
            pub enum TaskState {
                Draft,
                Review(String),
            }
        };

        let info = EnumInfo::from_item_enum_with_module(&item, StateModulePath("crate::workflow".into()))
            .expect("state metadata");
        let parsed = info.parse().expect("parsed state metadata");

        assert_eq!(parsed.vis.to_token_stream().to_string(), "pub");
        assert_eq!(parsed.derives.len(), 1);
        assert_eq!(parsed.variants.len(), 2);
        assert!(parsed.variants[0].data_type.is_none());
        assert_eq!(
            parsed.variants[1]
                .data_type
                .as_ref()
                .expect("review payload")
                .to_token_stream()
                .to_string(),
            "String"
        );
        assert_eq!(
            info.variants[1]
                .parse_data_type()
                .expect("variant payload parse")
                .expect("payload")
                .to_token_stream()
                .to_string(),
            "String"
        );
    }
}
