use macro_registry::callsite::{current_source_info, module_path_for_line};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use std::sync::{OnceLock, RwLock};
use syn::{Fields, Ident, Item, ItemEnum, Path, Type, Visibility};

use crate::{
    ItemTarget, ModulePath, SourceFingerprint, crate_root_for_file, current_crate_root,
    extract_derives, parse_present_attrs, source_file_fingerprint, PresentationAttr,
};

// Structure to hold extracted enum data
#[derive(Clone)]
#[allow(unused)]
pub struct EnumInfo {
    pub derives: Vec<String>,
    pub vis: String,
    pub name: String,
    pub variants: Vec<VariantInfo>,
    pub presentation: Option<PresentationAttr>,
    pub generics: String,
    pub module_path: StateModulePath,
    pub file_path: Option<String>,
    pub crate_root: Option<String>,
    pub file_fingerprint: Option<SourceFingerprint>,
    pub line_number: usize,
}

impl EnumInfo {
    pub fn get_variant_from_name(&self, variant_name: &str) -> Option<&VariantInfo> {
        self.variants
            .iter()
            .find(|v| v.name == variant_name || to_snake_case(&v.name) == variant_name)
    }
}

#[derive(Clone)]
pub struct VariantInfo {
    pub name: String,
    pub shape: VariantShape,
    pub presentation: Option<PresentationAttr>,
}

#[derive(Clone)]
pub enum VariantShape {
    Unit,
    Tuple { data_type: String },
    Named {
        data_struct_name: String,
        fields: Vec<NamedFieldInfo>,
    },
}

#[derive(Clone)]
pub struct NamedFieldInfo {
    pub name: String,
    pub field_type: String,
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
                shape: variant.parse_shape()?,
                presentation: variant.presentation.clone(),
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
        match &self.shape {
            VariantShape::Unit => Ok(None),
            VariantShape::Tuple { data_type } => syn::parse_str::<Type>(data_type)
                .map(Some)
                .map_err(|err| err.to_compile_error()),
            VariantShape::Named {
                data_struct_name, ..
            } => syn::parse_str::<Type>(data_struct_name)
                .map(Some)
                .map_err(|err| err.to_compile_error()),
        }
    }

    pub(crate) fn parse_shape(&self) -> Result<ParsedVariantShape, TokenStream> {
        match &self.shape {
            VariantShape::Unit => Ok(ParsedVariantShape::Unit),
            VariantShape::Tuple { data_type } => syn::parse_str::<Type>(data_type)
                .map(|data_type| ParsedVariantShape::Tuple {
                    data_type: Box::new(data_type),
                })
                .map_err(|err| err.to_compile_error()),
            VariantShape::Named {
                data_struct_name,
                fields,
            } => {
                let data_struct_ident = format_ident!("{}", data_struct_name);
                let mut parsed_fields = Vec::with_capacity(fields.len());
                for field in fields {
                    parsed_fields.push(ParsedNamedFieldInfo {
                        ident: format_ident!("{}", field.name),
                        field_type: syn::parse_str::<Type>(&field.field_type)
                            .map_err(|err| err.to_compile_error())?,
                    });
                }

                Ok(ParsedVariantShape::Named {
                    data_struct_ident,
                    fields: parsed_fields,
                })
            }
        }
    }
}

pub(crate) struct ParsedEnumInfo {
    pub(crate) vis: Visibility,
    pub(crate) derives: Vec<Path>,
    pub(crate) variants: Vec<ParsedVariantInfo>,
}

pub(crate) struct ParsedVariantInfo {
    pub(crate) name: String,
    pub(crate) shape: ParsedVariantShape,
    pub(crate) presentation: Option<PresentationAttr>,
}

pub(crate) enum ParsedVariantShape {
    Unit,
    Tuple { data_type: Box<Type> },
    Named {
        data_struct_ident: Ident,
        fields: Vec<ParsedNamedFieldInfo>,
    },
}

pub(crate) struct ParsedNamedFieldInfo {
    pub(crate) ident: Ident,
    pub(crate) field_type: Type,
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
            let variant_tokens = match &variant.shape {
                ParsedVariantShape::Unit => quote! { #var_name },
                ParsedVariantShape::Tuple { data_type } => quote! { #var_name(#data_type) },
                ParsedVariantShape::Named { fields, .. } => {
                    let named_fields = fields.iter().map(|field| {
                        let field_ident = &field.ident;
                        let field_type = &field.field_type;
                        quote! { #field_ident: #field_type }
                    });
                    quote! { #var_name { #(#named_fields),* } }
                }
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

static LOADED_STATE_ENUMS: OnceLock<RwLock<Vec<EnumInfo>>> = OnceLock::new();

#[derive(Clone)]
pub enum LoadedStateLookupFailure {
    NotFound,
    Ambiguous(Vec<EnumInfo>),
}

fn loaded_state_enums() -> &'static RwLock<Vec<EnumInfo>> {
    LOADED_STATE_ENUMS.get_or_init(|| RwLock::new(Vec::new()))
}

fn same_loaded_state(left: &EnumInfo, right: &EnumInfo) -> bool {
    left.name == right.name
        && left.module_path.as_ref() == right.module_path.as_ref()
        && left.file_path == right.file_path
        && left.line_number == right.line_number
}

fn upsert_loaded_state(enum_info: &EnumInfo) {
    let Ok(mut states) = loaded_state_enums().write() else {
        return;
    };

    if let Some(existing) = states
        .iter_mut()
        .find(|existing| same_loaded_state(existing, enum_info))
    {
        *existing = enum_info.clone();
    } else {
        states.push(enum_info.clone());
    }
}

fn loaded_state_candidates_matching<F>(matches: F) -> Vec<EnumInfo>
where
    F: Fn(&EnumInfo) -> bool,
{
    let current_crate_root = current_crate_root();
    let Ok(states) = loaded_state_enums().read() else {
        return Vec::new();
    };

    states
        .iter()
        .filter(|state| loaded_state_is_current(state, current_crate_root.as_deref()))
        .filter(|state| matches(state))
        .cloned()
        .collect()
}

fn loaded_state_is_current(state: &EnumInfo, current_crate_root: Option<&str>) -> bool {
    if current_crate_root.is_some() && state.crate_root.as_deref() != current_crate_root {
        return false;
    }

    match (state.file_path.as_deref(), state.file_fingerprint.as_ref()) {
        (Some(file_path), Some(fingerprint)) => {
            source_file_fingerprint(file_path).as_ref() == Some(fingerprint)
        }
        _ => true,
    }
}

fn lookup_loaded_state_candidates(
    candidates: Vec<EnumInfo>,
) -> Result<EnumInfo, LoadedStateLookupFailure> {
    match candidates.len() {
        0 => Err(LoadedStateLookupFailure::NotFound),
        1 => Ok(candidates.into_iter().next().expect("single candidate")),
        _ => Err(LoadedStateLookupFailure::Ambiguous(candidates)),
    }
}

pub fn lookup_loaded_state_enum(
    enum_path: &StateModulePath,
) -> Result<EnumInfo, LoadedStateLookupFailure> {
    lookup_loaded_state_candidates(loaded_state_candidates_matching(|state| {
        state.module_path.as_ref() == enum_path.as_ref()
    }))
}

pub fn lookup_loaded_state_enum_by_name(
    enum_path: &StateModulePath,
    enum_name: &str,
) -> Result<EnumInfo, LoadedStateLookupFailure> {
    lookup_loaded_state_candidates(loaded_state_candidates_matching(|state| {
        state.module_path.as_ref() == enum_path.as_ref() && state.name == enum_name
    }))
}

pub fn format_loaded_state_candidates(candidates: &[EnumInfo]) -> String {
    candidates
        .iter()
        .map(|candidate| {
            let file_path = candidate.file_path.as_deref().unwrap_or("<unknown file>");
            format!(
                "`{}` in `{}` ({file_path}:{})",
                candidate.name, candidate.module_path, candidate.line_number
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
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
        Self::from_item_enum_with_module_and_file(
            item,
            module_path.into(),
            Some(file_path),
            line_number,
        )
    }

    #[cfg(test)]
    pub fn from_item_enum_with_module(
        item: &ItemEnum,
        module_path: StateModulePath,
    ) -> syn::Result<Self> {
        let file_path = current_source_info().map(|(path, _)| path);
        let line_number = current_source_info().map(|(_, line)| line).unwrap_or_default();
        Self::from_item_enum_with_module_and_file(item, module_path, file_path, line_number)
    }

    fn from_item_enum_with_module_and_file(
        item: &ItemEnum,
        module_path: StateModulePath,
        file_path: Option<String>,
        line_number: usize,
    ) -> syn::Result<Self> {
        validate_state_enum_shape(item)?;
        let crate_root = file_path
            .as_deref()
            .and_then(crate_root_for_file);
        let file_fingerprint = file_path
            .as_deref()
            .and_then(source_file_fingerprint);

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
        let presentation = parse_present_attrs(&item.attrs)?;

        let mut variants = Vec::new();
        for variant in &item.variants {
            let name = variant.ident.to_string();
            let shape = match &variant.fields {
                Fields::Unnamed(fields) => VariantShape::Tuple {
                    data_type: fields
                        .unnamed
                        .first()
                        .expect("validated state tuple field")
                        .ty
                        .to_token_stream()
                        .to_string(),
                },
                Fields::Unit => VariantShape::Unit,
                Fields::Named(fields) => VariantShape::Named {
                    data_struct_name: format!("{}Data", variant.ident),
                    fields: fields
                        .named
                        .iter()
                        .filter_map(|field| {
                            field.ident.as_ref().map(|ident| NamedFieldInfo {
                                name: ident.to_string(),
                                field_type: field.ty.to_token_stream().to_string(),
                            })
                        })
                        .collect(),
                },
            };
            let presentation = parse_present_attrs(&variant.attrs)?;

            variants.push(VariantInfo {
                name,
                shape,
                presentation,
            });
        }

        Ok(Self {
            derives,
            vis,
            name,
            variants,
            presentation,
            generics,
            module_path,
            file_path,
            crate_root,
            file_fingerprint,
            line_number,
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
            Fields::Named(fields) if fields.named.is_empty() => {
                let variant_name = variant.ident.to_string();
                return Err(syn::Error::new_spanned(
                    variant,
                    format!(
                        "Error: #[state] enum `{enum_name}` variant `{variant_name}` uses empty named fields.\nFix: use `{variant_name}` for a unit state or add at least one named field."
                    ),
                ));
            }
            Fields::Named(_) => {}
        }
    }

    Ok(())
}

pub fn generate_state_impls(enum_info: &EnumInfo) -> proc_macro2::TokenStream {
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

        let tokens = match &variant.shape {
            ParsedVariantShape::Unit => {
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
            ParsedVariantShape::Tuple { data_type } => {
                quote! {
                    #variant_derives
                    #vis struct #variant_name (pub #data_type);

                    impl #state_trait_ident for #variant_name {
                        type Data = #data_type;
                    }

                    impl statum::StateMarker for #variant_name {
                        type Data = #data_type;
                    }

                    impl statum::DataState for #variant_name {}
                }
            }
            ParsedVariantShape::Named {
                data_struct_ident,
                fields,
            } => {
                let payload_fields = fields.iter().map(|field| {
                    let field_ident = &field.ident;
                    let field_type = &field.field_type;
                    quote! { pub #field_ident: #field_type }
                });

                quote! {
                    #variant_derives
                    #vis struct #data_struct_ident {
                        #(#payload_fields),*
                    }

                    #variant_derives
                    #vis struct #variant_name (pub #data_struct_ident);

                    impl #state_trait_ident for #variant_name {
                        type Data = #data_struct_ident;
                    }

                    impl statum::StateMarker for #variant_name {
                        type Data = #data_struct_ident;
                    }

                    impl statum::DataState for #variant_name {}
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
    upsert_loaded_state(enum_info);
}

#[cfg(test)]
mod tests {
    use quote::ToTokens;
    use syn::parse_quote;

    use super::{EnumInfo, ParsedVariantShape, StateModulePath, VariantShape};

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
        assert!(matches!(parsed.variants[0].shape, ParsedVariantShape::Unit));
        let ParsedVariantShape::Tuple { ref data_type } = parsed.variants[1].shape else {
            panic!("expected tuple variant");
        };
        assert_eq!(data_type.to_token_stream().to_string(), "String");
        assert_eq!(
            info.variants[1]
                .parse_data_type()
                .expect("variant payload parse")
                .expect("payload")
                .to_token_stream()
                .to_string(),
            "String"
        );
        assert!(matches!(info.variants[0].shape, VariantShape::Unit));
    }

    #[test]
    fn parse_named_variants_into_generated_payloads() {
        let item: syn::ItemEnum = parse_quote! {
            pub enum TaskState {
                Review {
                    reviewer: String,
                    priority: u8,
                },
            }
        };

        let module_path: StateModulePath = crate::ModulePath("crate::workflow".into());
        let info =
            EnumInfo::from_item_enum_with_module(&item, module_path).expect("state metadata");
        let parsed = info.parse().expect("parsed state metadata");

        let VariantShape::Named {
            data_struct_name,
            fields,
        } = &info.variants[0].shape
        else {
            panic!("expected named variant");
        };
        assert_eq!(data_struct_name, "ReviewData");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "reviewer");
        assert_eq!(fields[1].name, "priority");
        assert_eq!(
            info.variants[0]
                .parse_data_type()
                .expect("named payload type")
                .expect("named payload")
                .to_token_stream()
                .to_string(),
            "ReviewData"
        );

        let ParsedVariantShape::Named {
            ref data_struct_ident,
            ref fields,
        } = parsed.variants[0].shape
        else {
            panic!("expected parsed named variant");
        };
        assert_eq!(data_struct_ident.to_string(), "ReviewData");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].ident.to_string(), "reviewer");
        assert_eq!(fields[0].field_type.to_token_stream().to_string(), "String");
    }
}
