use macro_registry::callsite::{current_source_file, current_source_info, module_path_for_line};
use macro_registry::query;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::{Generics, Ident, ItemStruct, LitStr, Type, Visibility};

use crate::{
    EnumInfo, LoadedStateLookupFailure, ModulePath, SourceFingerprint, StateModulePath,
    crate_root_for_file, extract_derives, format_loaded_state_candidates,
    lookup_loaded_state_enum, lookup_loaded_state_enum_by_name, source_file_fingerprint,
    parse_present_attrs, parse_presentation_types_attr, PresentationAttr,
    PresentationTypesAttr,
};
use super::extra_type_arguments_tokens;

pub type MachinePath = ModulePath;

#[derive(Clone)]
pub struct MachineInfo {
    pub name: String,
    pub vis: String,
    pub derives: Vec<String>,
    pub fields: Vec<MachineField>,
    pub presentation: Option<PresentationAttr>,
    pub presentation_types: Option<PresentationTypesAttr>,
    pub module_path: MachinePath,
    pub line_number: usize,
    pub generics: String,
    pub state_generic_name: Option<String>,
    pub file_path: Option<String>,
    pub crate_root: Option<String>,
    pub file_fingerprint: Option<SourceFingerprint>,
}

impl MachineInfo {
    pub fn field_names(&self) -> Vec<Ident> {
        self.fields
            .iter()
            .map(|field| format_ident!("{}", field.name))
            .collect()
    }

    pub(crate) fn parse(&self) -> Result<ParsedMachineInfo, TokenStream> {
        let vis = syn::parse_str::<Visibility>(&self.vis).map_err(|err| err.to_compile_error())?;
        let generics =
            syn::parse_str::<Generics>(&self.generics).map_err(|err| err.to_compile_error())?;
        let mut derives = Vec::with_capacity(self.derives.len());
        for derive in &self.derives {
            derives.push(syn::parse_str::<syn::Path>(derive).map_err(|err| err.to_compile_error())?);
        }

        let mut fields = Vec::with_capacity(self.fields.len());
        let extra_type_arguments = extra_type_arguments_tokens(&generics);
        for field in &self.fields {
            let ident = format_ident!("{}", field.name);
            let vis =
                syn::parse_str::<Visibility>(&field.vis).map_err(|err| err.to_compile_error())?;
            let alias_ident = format_ident!("{}", field.field_type);
            let field_type = syn::parse2::<Type>(quote! { #alias_ident #extra_type_arguments })
                .map_err(|err| err.to_compile_error())?;
            fields.push(ParsedMachineField {
                ident,
                vis,
                field_type,
            });
        }

        Ok(ParsedMachineInfo {
            vis,
            derives,
            fields,
            generics,
        })
    }

    pub fn get_matching_state_enum(&self) -> Result<EnumInfo, TokenStream> {
        let state_path: StateModulePath = self.module_path.clone();
        let state_enum = if let Some(expected_name) = self.state_generic_name.as_deref() {
            lookup_loaded_state_enum_by_name(&state_path, expected_name)
        } else {
            lookup_loaded_state_enum(&state_path)
        };
        state_enum.map_err(|failure| missing_state_enum_error(self, failure))
    }

    pub fn from_item_struct(item: &ItemStruct) -> syn::Result<Self> {
        let Some(file_path) = current_source_file() else {
            return Err(syn::Error::new(
                item.ident.span(),
                format!(
                    "Internal error: could not read source information for `#[machine]` struct `{}`.",
                    item.ident
                ),
            ));
        };
        let line_number = item.ident.span().start().line;
        let Some(module_path) = module_path_for_line(&file_path, line_number) else {
            return Err(syn::Error::new(
                item.ident.span(),
                format!(
                    "Internal error: could not resolve the module path for `#[machine]` struct `{}`.",
                    item.ident
                ),
            ));
        };
        let module_path: MachinePath = module_path.into();
        let fields = collect_fields(item);
        let crate_root = crate_root_for_file(&file_path);
        let file_fingerprint = source_file_fingerprint(&file_path);

        Ok(Self {
            name: item.ident.to_string(),
            vis: item.vis.to_token_stream().to_string(),
            derives: item
                .attrs
                .iter()
                .filter_map(extract_derives)
                .flatten()
                .collect(),
            presentation: parse_present_attrs(&item.attrs)?,
            presentation_types: parse_presentation_types_attr(&item.attrs)?,
            module_path,
            line_number,
            fields,
            generics: item.generics.to_token_stream().to_string(),
            state_generic_name: extract_state_generic_name(&item.generics),
            file_path: Some(file_path),
            crate_root,
            file_fingerprint,
        })
    }

    #[cfg(test)]
    pub fn from_item_struct_with_module(item: &ItemStruct, module_path: &MachinePath) -> Option<Self> {
        if item.generics.params.is_empty() {
            return None;
        }

        let line_number = item.ident.span().start().line;
        let file_path = current_source_file();
        let presentation = parse_present_attrs(&item.attrs).ok()?;
        let presentation_types = parse_presentation_types_attr(&item.attrs).ok()?;
        Some(Self {
            name: item.ident.to_string(),
            vis: item.vis.to_token_stream().to_string(),
            derives: item
                .attrs
                .iter()
                .filter_map(extract_derives)
                .flatten()
                .collect(),
            presentation,
            presentation_types,
            fields: collect_fields(item),
            module_path: module_path.clone(),
            line_number,
            generics: item.generics.to_token_stream().to_string(),
            state_generic_name: extract_state_generic_name(&item.generics),
            crate_root: file_path
                .as_deref()
                .and_then(crate_root_for_file),
            file_fingerprint: file_path
                .as_deref()
                .and_then(source_file_fingerprint),
            file_path,
        })
    }
}

#[derive(Clone)]
pub struct MachineField {
    pub name: String,
    pub vis: String,
    pub field_type: String,
}

pub(crate) struct ParsedMachineInfo {
    pub(crate) vis: Visibility,
    pub(crate) derives: Vec<syn::Path>,
    pub(crate) fields: Vec<ParsedMachineField>,
    pub(crate) generics: Generics,
}

impl ParsedMachineInfo {
    pub(crate) fn field_idents_and_types(&self) -> Vec<(Ident, Type)> {
        self.fields
            .iter()
            .map(|field| (field.ident.clone(), field.field_type.clone()))
            .collect()
    }
}

pub(crate) struct ParsedMachineField {
    pub(crate) ident: Ident,
    pub(crate) vis: Visibility,
    pub(crate) field_type: Type,
}

pub(super) fn is_rust_analyzer() -> bool {
    std::env::var("RUST_ANALYZER_INTERNALS").is_ok()
}

pub(super) fn field_type_alias_name(machine_name: &str, field_name: &str) -> String {
    let field_name = field_name.trim_start_matches("r#");
    format!(
        "__statum_{}_{}_field_type",
        crate::to_snake_case(machine_name),
        field_name
    )
}

fn collect_fields(item: &ItemStruct) -> Vec<MachineField> {
    let machine_name = item.ident.to_string();
    item.fields
        .iter()
        .filter_map(|field| {
            field.ident.as_ref().map(|ident| MachineField {
                name: ident.to_string(),
                vis: field.vis.to_token_stream().to_string(),
                field_type: field_type_alias_name(&machine_name, &ident.to_string()),
            })
        })
        .collect()
}

fn extract_state_generic_name(generics: &Generics) -> Option<String> {
    let first_param = generics.params.first()?;
    if let syn::GenericParam::Type(ty) = first_param {
        Some(ty.ident.to_string())
    } else {
        None
    }
}

fn missing_state_enum_error(
    machine_info: &MachineInfo,
    failure: LoadedStateLookupFailure,
) -> TokenStream {
    if is_rust_analyzer() {
        return TokenStream::new();
    }

    let expected = machine_info.state_generic_name.as_deref();
    let expected_line = expected
        .map(|name| format!("Expected a `#[state]` enum named `{name}` in module `{}`.", machine_info.module_path))
        .unwrap_or_else(|| {
            format!(
                "Could not infer the expected `#[state]` enum name from machine `{}`.",
                machine_info.name
            )
        });
    let available = available_state_candidates_in_module(
        machine_info.file_path.as_deref(),
        machine_info.module_path.as_ref(),
    );
    let available_line = if available.is_empty() {
        "No `#[state]` enums were found in that module.".to_string()
    } else {
        format!(
            "Available `#[state]` enums in that module: {}.",
            query::format_candidates(&available)
        )
    };
    let ordering_line = expected.and_then(|name| {
        available
            .iter()
            .find(|candidate| {
                candidate.name == name && candidate.line_number > machine_info.line_number
            })
            .map(|candidate| {
                format!(
                    "Source scan found `#[state]` enum `{name}` later in this module on line {}. If that item is active for this build, move it above machine `{}` because Statum resolves these relationships in expansion order.",
                    candidate.line_number, machine_info.name
                )
            })
    });
    let ordering_line = ordering_line
        .map(|line| format!("{line}\n"))
        .unwrap_or_default();
    let elsewhere_line = expected
        .and_then(|name| {
            same_named_state_candidates_elsewhere(
                machine_info.file_path.as_deref(),
                name,
                machine_info.module_path.as_ref(),
            )
        })
        .map(|candidates| {
            format!(
                "Same-named `#[state]` enums elsewhere in this file: {}.",
                query::format_candidates(&candidates)
            )
        })
        .unwrap_or_else(|| "No same-named `#[state]` enums were found in other modules of this file.".to_string());
    let missing_attr_line = expected.and_then(|name| {
        plain_enum_line_in_module(machine_info.file_path.as_deref(), machine_info.module_path.as_ref(), name).map(|line| {
            format!("An enum named `{name}` exists on line {line}, but it is not annotated with `#[state]`.")
        })
    });
    let authority_line = match failure {
        LoadedStateLookupFailure::NotFound => {
            "Statum only resolves `#[state]` enums that have already expanded before this `#[machine]` declaration.".to_string()
        }
        LoadedStateLookupFailure::Ambiguous(candidates) => format!(
            "Loaded `#[state]` candidates were ambiguous: {}.",
            format_loaded_state_candidates(&candidates)
        ),
    };
    let message = format!(
        "Failed to resolve the #[state] enum for machine `{}`.\n{}\n{}\n{}{}\n{}\n{}\nHelp: make sure the machine's first generic names the right `#[state]` enum in this module and declare that `#[state]` enum before the machine.\nCorrect shape: `struct {}<ExpectedState> {{ ... }}` where `ExpectedState` is a `#[state]` enum in `{}`.",
        machine_info.name,
        expected_line,
        authority_line,
        ordering_line,
        missing_attr_line.unwrap_or_else(|| "No plain enum with that expected name was found in that module either.".to_string()),
        elsewhere_line,
        available_line,
        machine_info.name,
        machine_info.module_path,
    );
    let message = LitStr::new(&message, Span::call_site());
    quote! { compile_error!(#message); }
}

fn available_state_candidates_in_module(
    file_path: Option<&str>,
    module_path: &str,
) -> Vec<query::ItemCandidate> {
    let Some(file_path) = file_path
        .map(str::to_owned)
        .or_else(|| current_source_info().map(|(path, _)| path))
    else {
        return Vec::new();
    };
    query::candidates_in_module(&file_path, module_path, query::ItemKind::Enum, Some("state"))
}

fn plain_enum_line_in_module(
    file_path: Option<&str>,
    module_path: &str,
    enum_name: &str,
) -> Option<usize> {
    let file_path = file_path
        .map(str::to_owned)
        .or_else(|| current_source_info().map(|(path, _)| path))?;
    query::plain_item_line_in_module(
        &file_path,
        module_path,
        query::ItemKind::Enum,
        enum_name,
        Some("state"),
    )
}

fn same_named_state_candidates_elsewhere(
    file_path: Option<&str>,
    enum_name: &str,
    module_path: &str,
) -> Option<Vec<query::ItemCandidate>> {
    let file_path = file_path
        .map(str::to_owned)
        .or_else(|| current_source_info().map(|(path, _)| path))?;
    let candidates = query::same_named_candidates_elsewhere(
        &file_path,
        module_path,
        query::ItemKind::Enum,
        enum_name,
        Some("state"),
    );
    (!candidates.is_empty()).then_some(candidates)
}

#[cfg(test)]
mod tests {
    use quote::ToTokens;
    use syn::parse_quote;

    use super::{MachineInfo, MachinePath};

    #[test]
    fn parse_round_trips_machine_shape() {
        let item: syn::ItemStruct = parse_quote! {
            #[derive(Clone)]
            pub struct TaskMachine<TaskState> {
                pub client: String,
                priority: u32,
            }
        };

        let module_path: MachinePath = crate::ModulePath("crate::workflow".into());
        let info =
            MachineInfo::from_item_struct_with_module(&item, &module_path).expect("machine metadata");
        let parsed = info.parse().expect("parsed machine metadata");

        assert_eq!(info.state_generic_name.as_deref(), Some("TaskState"));
        assert_eq!(info.fields[0].field_type, "__statum_task_machine_client_field_type");
        assert_eq!(parsed.generics.to_token_stream().to_string(), "< TaskState >");
        assert_eq!(parsed.derives.len(), 1);
        assert_eq!(parsed.fields.len(), 2);
        assert_eq!(parsed.fields[0].ident.to_string(), "client");
        assert_eq!(
            parsed.fields[0].field_type.to_token_stream().to_string(),
            "__statum_task_machine_client_field_type"
        );
        assert_eq!(parsed.fields[1].ident.to_string(), "priority");
        assert_eq!(parsed.fields[1].vis.to_token_stream().to_string(), "");
    }
}
