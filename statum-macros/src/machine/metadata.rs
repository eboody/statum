use macro_registry::callsite::{current_source_info, module_path_for_line};
use macro_registry::registry::SourceContext;
use macro_registry::query;
use macro_registry::registry;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::{Generics, Ident, ItemStruct, LitStr, Type, Visibility};

use crate::{
    EnumInfo, ModulePath, StateModulePath, ensure_state_enum_loaded,
    ensure_state_enum_loaded_by_name, ensure_state_enum_loaded_by_name_from_source,
    ensure_state_enum_loaded_from_source, extract_derives,
};

pub type MachinePath = ModulePath;

#[derive(Clone)]
pub struct MachineInfo {
    pub name: String,
    pub vis: String,
    pub derives: Vec<String>,
    pub fields: Vec<MachineField>,
    pub module_path: MachinePath,
    pub line_number: usize,
    pub generics: String,
    pub state_generic_name: Option<String>,
    pub file_path: Option<String>,
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
        for field in &self.fields {
            let ident = format_ident!("{}", field.name);
            let vis =
                syn::parse_str::<Visibility>(&field.vis).map_err(|err| err.to_compile_error())?;
            let field_type =
                syn::parse_str::<Type>(&field.field_type).map_err(|err| err.to_compile_error())?;
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
        // Included transition fragments must resolve the state enum against the
        // machine's source file, not the include file's pseudo-module context.
        let source = self
            .file_path
            .as_ref()
            .map(|file_path| SourceContext::new(file_path.clone(), self.line_number));
        let state_enum = if let Some(expected_name) = self.state_generic_name.as_deref() {
            source
                .as_ref()
                .and_then(|source| {
                    ensure_state_enum_loaded_by_name_from_source(&state_path, expected_name, source)
                })
                .or_else(|| ensure_state_enum_loaded_by_name(&state_path, expected_name))
        } else {
            source
                .as_ref()
                .and_then(|source| ensure_state_enum_loaded_from_source(&state_path, source))
                .or_else(|| ensure_state_enum_loaded(&state_path))
        };
        let Some(state_enum) = state_enum else {
            return Err(missing_state_enum_error(self));
        };
        Ok(state_enum)
    }

    pub fn from_item_struct(item: &ItemStruct) -> syn::Result<Self> {
        let Some((file_path, line_number)) = current_source_info() else {
            return Err(syn::Error::new(
                item.ident.span(),
                format!(
                    "Internal error: could not read source information for `#[machine]` struct `{}`.",
                    item.ident
                ),
            ));
        };
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

        Ok(Self {
            name: item.ident.to_string(),
            vis: item.vis.to_token_stream().to_string(),
            derives: item
                .attrs
                .iter()
                .filter_map(extract_derives)
                .flatten()
                .collect(),
            module_path,
            line_number,
            fields,
            generics: item.generics.to_token_stream().to_string(),
            state_generic_name: extract_state_generic_name(&item.generics),
            file_path: Some(file_path),
        })
    }

    pub fn from_item_struct_with_module(item: &ItemStruct, module_path: &MachinePath) -> Option<Self> {
        if item.generics.params.is_empty() {
            return None;
        }

        let line_number = current_source_info().map(|(_, line)| line).unwrap_or_default();
        Some(Self {
            name: item.ident.to_string(),
            vis: item.vis.to_token_stream().to_string(),
            derives: item
                .attrs
                .iter()
                .filter_map(extract_derives)
                .flatten()
                .collect(),
            fields: collect_fields(item),
            module_path: module_path.clone(),
            line_number,
            generics: item.generics.to_token_stream().to_string(),
            state_generic_name: extract_state_generic_name(&item.generics),
            file_path: current_source_info().map(|(path, _)| path),
        })
    }
}

impl registry::RegistryValue for MachineInfo {
    fn file_path(&self) -> Option<&str> {
        self.file_path.as_deref()
    }

    fn set_file_path(&mut self, file_path: String) {
        self.file_path = Some(file_path);
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

fn missing_state_enum_error(machine_info: &MachineInfo) -> TokenStream {
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
    let message = format!(
        "Failed to resolve the #[state] enum for machine `{}`.\n{}\n{}\n{}\n{}\nHelp: make sure the machine's first generic names the right `#[state]` enum in this module.\nCorrect shape: `struct {}<ExpectedState> {{ ... }}` where `ExpectedState` is a `#[state]` enum in `{}`.",
        machine_info.name,
        expected_line,
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
