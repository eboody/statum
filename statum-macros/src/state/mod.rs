//! `#[state]` subsystem: parse enums, validate legal shapes, store registry facts, and emit markers.

mod emission;
mod registry;
mod validation;

use crate::source::{module_path_for_line, source_info_for_span};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use std::marker::PhantomData;
use syn::{Fields, Ident, ItemEnum, Path, Type, Visibility};

use crate::{
    ModulePath, SourceFingerprint, crate_root_for_file, extract_derives, parse_present_attrs,
    source_file_fingerprint, PresentationAttr,
};

pub use emission::generate_state_impls;
pub use registry::{
    format_loaded_state_candidates, lookup_loaded_state_enum, lookup_loaded_state_enum_by_name,
    store_state_enum,
};
pub use validation::{invalid_state_target_error, validate_state_enum};

pub fn expand_state(input: ItemEnum) -> TokenStream {
    StateExpansionBuilder::<ParsedStatePhase>::parse(input)
        .and_then(StateExpansionBuilder::<ParsedStatePhase>::validate)
        .map(StateExpansionBuilder::<ValidatedStatePhase>::register)
        .map(StateExpansionBuilder::<RegisteredStatePhase>::emit)
        .unwrap_or_else(|err| err)
}

struct ParsedStatePhase;
struct ValidatedStatePhase;
struct RegisteredStatePhase;

struct StateExpansionBuilder<State> {
    input: ItemEnum,
    enum_info: Option<EnumInfo>,
    _state: PhantomData<State>,
}

impl StateExpansionBuilder<ParsedStatePhase> {
    fn parse(input: ItemEnum) -> Result<Self, TokenStream> {
        let enum_info = EnumInfo::from_item_enum(&input).map_err(|err| err.to_compile_error())?;
        Ok(Self {
            input,
            enum_info: Some(enum_info),
            _state: PhantomData,
        })
    }

    fn validate(self) -> Result<StateExpansionBuilder<ValidatedStatePhase>, TokenStream> {
        if let Some(error) = validate_state_enum(&self.input) {
            return Err(error);
        }

        Ok(StateExpansionBuilder {
            input: self.input,
            enum_info: self.enum_info,
            _state: PhantomData,
        })
    }
}

impl StateExpansionBuilder<ValidatedStatePhase> {
    fn register(self) -> StateExpansionBuilder<RegisteredStatePhase> {
        if let Some(enum_info) = self.enum_info.as_ref() {
            store_state_enum(enum_info);
        }

        StateExpansionBuilder {
            input: self.input,
            enum_info: self.enum_info,
            _state: PhantomData,
        }
    }
}

impl StateExpansionBuilder<RegisteredStatePhase> {
    fn emit(self) -> TokenStream {
        match self.enum_info {
            Some(enum_info) => generate_state_impls(&enum_info),
            None => {
                let message = crate::diagnostics::DiagnosticMessage::new(format!(
                    "internal Statum error: registered `#[state]` pipeline for `{}` reached emission without enum metadata.",
                    self.input.ident
                ))
                .render();
                syn::Error::new_spanned(&self.input.ident, message).to_compile_error()
            }
        }
    }
}

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

#[derive(Clone)]
pub enum LoadedStateLookupFailure {
    NotFound,
    Ambiguous(Vec<EnumInfo>),
}

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

impl EnumInfo {
    pub fn from_item_enum(item: &ItemEnum) -> syn::Result<Self> {
        let line_number = item.ident.span().start().line;
        let Some((file_path, line_number)) = source_info_for_span(item.ident.span()) else {
            return Self::from_item_enum_with_module_and_file(item, "crate".into(), None, line_number);
        };
        let Some(module_path) = module_path_for_line(&file_path, line_number) else {
            if crate::machine::is_rust_analyzer() {
                return Self::from_item_enum_with_module_and_file(
                    item,
                    "crate".into(),
                    None,
                    line_number,
                );
            }
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
        let file_path = crate::source::current_source_info().map(|(file_path, _)| file_path);
        let line_number = item.ident.span().start().line;
        Self::from_item_enum_with_module_and_file(item, module_path, file_path, line_number)
    }

    fn from_item_enum_with_module_and_file(
        item: &ItemEnum,
        module_path: StateModulePath,
        file_path: Option<String>,
        line_number: usize,
    ) -> syn::Result<Self> {
        validation::validate_state_enum_shape(item)?;
        let crate_root = file_path.as_deref().and_then(crate_root_for_file);
        let file_fingerprint = file_path.as_deref().and_then(source_file_fingerprint);

        let name = item.ident.to_string();
        let vis = item.vis.to_token_stream().to_string();
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
