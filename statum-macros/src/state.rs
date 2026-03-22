use macro_registry::analysis::{EnumEntry, FileAnalysis};
use macro_registry::callsite::{current_source_info, module_path_for_line};
use macro_registry::registry;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{Fields, Ident, Item, ItemEnum, Path, Type, Visibility};

use crate::{ItemTarget, ModulePath, extract_derives};

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

impl registry::RegistryValue for EnumInfo {
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

pub type StateModulePath = ModulePath;

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

static STATE_ENUMS: registry::StaticRegistry<StateModulePath, EnumInfo> =
    registry::StaticRegistry::new();

struct StateRegistryDomain;

impl registry::RegistryDomain for StateRegistryDomain {
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

    fn entry_hint(entry: &Self::Entry) -> Option<String> {
        Some(entry.item.ident.to_string())
    }
}

impl registry::NamedRegistryDomain for StateRegistryDomain {
    fn entry_name(entry: &Self::Entry) -> String {
        entry.item.ident.to_string()
    }

    fn value_name(value: &Self::Value) -> String {
        value.name.clone()
    }
}

pub fn get_state_enum(enum_path: &StateModulePath) -> Option<EnumInfo> {
    STATE_ENUMS.get_cloned(enum_path)
}

pub fn invalid_state_target_error(item: &Item) -> TokenStream {
    let target = ItemTarget::from(item);
    let message = match target.name() {
        Some(name) => format!(
            "Error: #[state] must be applied to an enum, but `{name}` is {} {}.\nFix: declare `enum {name} {{ ... }}` with unit variants like `Draft` or single-payload variants like `InReview(ReviewData)`, or remove `#[state]`.",
            target.article(),
            target.kind(),
        ),
        None => format!(
            "Error: #[state] must be applied to an enum, but this item is {} {}.\nFix: apply `#[state]` to an enum with unit variants like `Draft` or single-payload variants like `InReview(ReviewData)`.",
            target.article(),
            target.kind(),
        ),
    };
    syn::Error::new(target.span(), message).to_compile_error()
}

pub fn ensure_state_enum_loaded(enum_path: &StateModulePath) -> Option<EnumInfo> {
    registry::ensure_loaded::<StateRegistryDomain>(&STATE_ENUMS, enum_path)
}

pub fn ensure_state_enum_loaded_from_source(
    enum_path: &StateModulePath,
    source: &registry::SourceContext,
) -> Option<EnumInfo> {
    registry::try_ensure_loaded_from_source::<StateRegistryDomain>(
        &STATE_ENUMS,
        registry::LookupMode::from_key(enum_path),
        source,
    )
    .ok()
    .map(|loaded| loaded.value)
}

pub fn ensure_state_enum_loaded_by_name(
    enum_path: &StateModulePath,
    enum_name: &str,
) -> Option<EnumInfo> {
    registry::ensure_loaded_by_name::<StateRegistryDomain>(&STATE_ENUMS, enum_path, enum_name)
}

pub fn ensure_state_enum_loaded_by_name_from_source(
    enum_path: &StateModulePath,
    enum_name: &str,
    source: &registry::SourceContext,
) -> Option<EnumInfo> {
    registry::try_ensure_loaded_by_name_from_source::<StateRegistryDomain>(
        &STATE_ENUMS,
        registry::LookupMode::from_key(enum_path),
        enum_name,
        source,
    )
    .ok()
    .map(|loaded| loaded.value)
}
impl EnumInfo {
    pub fn from_item_enum(item: &ItemEnum) -> syn::Result<Self> {
        let Some((file_path, line_number)) = current_source_info() else {
            return Err(syn::Error::new(
                item.ident.span(),
                format!(
                    "Internal error: could not read source information for `#[state]` enum `{}`.",
                    item.ident
                ),
            ));
        };
        let Some(module_path) = module_path_for_line(&file_path, line_number) else {
            return Err(syn::Error::new(
                item.ident.span(),
                format!(
                    "Internal error: could not resolve the module path for `#[state]` enum `{}`.",
                    item.ident
                ),
            ));
        };
        Self::from_item_enum_with_module_and_file(item, module_path.into(), Some(file_path))
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
        validate_state_enum_shape(item)?;

        let name = item.ident.to_string();
        let vis = item.vis.to_token_stream().to_string();
        // 1.0 policy: generics on `#[state]` enums are intentionally unsupported.
        // `validate_state_enum` emits a compile error when generics are present.
        let generics = item.generics.clone().to_token_stream().to_string();

        let derives = item
            .attrs
            .iter()
            .filter_map(extract_derives)
            .flatten()
            .collect();

        let mut variants = Vec::new();
        for variant in &item.variants {
            let name = variant.ident.to_string();
            let data_type = match &variant.fields {
                Fields::Unnamed(fields) => fields
                    .unnamed
                    .first()
                    .map(|first| first.ty.to_token_stream().to_string()),
                Fields::Unit => None,
                Fields::Named(_) => unreachable!("state shape already validated"),
            };

            variants.push(VariantInfo { name, data_type });
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

fn validate_state_enum_shape(item: &ItemEnum) -> syn::Result<()> {
    let enum_name = item.ident.to_string();

    if !item.generics.params.is_empty() {
        let generics_display = item.generics.to_token_stream().to_string();
        return Err(syn::Error::new_spanned(
            &item.generics,
            format!(
                "Error: #[state] enum `{enum_name}` cannot declare generics.\nFix: keep `{enum_name}` non-generic and move generic data into payload types.\nFound: `enum {enum_name}{generics_display} {{ ... }}`."
            ),
        ));
    }

    if item.variants.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.ident,
            format!(
                "Error: #[state] enum `{enum_name}` must declare at least one variant.\nFix: add unit variants like `Draft` or single-payload variants like `InReview(ReviewData)`."
            ),
        ));
    }

    for variant in &item.variants {
        match &variant.fields {
            Fields::Unit => {}
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {}
            Fields::Unnamed(fields) => {
                let variant_name = variant.ident.to_string();
                let field_count = fields.unnamed.len();
                return Err(syn::Error::new_spanned(
                    variant,
                    format!(
                        "Error: #[state] enum `{enum_name}` variant `{variant_name}` carries {field_count} fields, but Statum supports at most one payload type per state.\nFix: wrap those fields in a separate payload type and use `{variant_name}({variant_name}Data)`."
                    ),
                ));
            }
            Fields::Named(_) => {
                let variant_name = variant.ident.to_string();
                return Err(syn::Error::new_spanned(
                    variant,
                    format!(
                        "Error: #[state] enum `{enum_name}` variant `{variant_name}` uses named fields, but Statum state variants must be unit variants like `{variant_name}` or single-payload tuple variants like `{variant_name}({variant_name}Data)`.\nFix: move the named fields into a payload type and reference that type as the only tuple field."
                    ),
                ));
            }
        }
    }

    Ok(())
}

pub fn generate_state_impls(enum_path: &StateModulePath) -> proc_macro2::TokenStream {
    let Some(enum_info) = get_state_enum(enum_path) else {
        let message = format!(
            "Internal error: state metadata for module `{}` was not cached during code generation.\nEnsure `#[state]` is applied in that module and try re-running `cargo check`.",
            enum_path
        );
        return quote! {
            compile_error!(#message);
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
    validate_state_enum_shape(item).err().map(|err| err.to_compile_error())
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

        let module_path: StateModulePath = crate::ModulePath("crate::workflow".into());
        let info =
            EnumInfo::from_item_enum_with_module(&item, module_path).expect("state metadata");
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
