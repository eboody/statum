use macro_registry::analysis::get_file_analysis;
use macro_registry::callsite::module_path_for_line;
use proc_macro2::TokenStream;

use crate::transition::parse_transition_impl;

use super::metadata::is_rust_analyzer;
use super::MachineInfo;

pub(crate) struct TransitionSite {
    pub(crate) method_name: String,
    pub(crate) source_state: String,
    pub(crate) target_states: Vec<String>,
}

pub(crate) fn collect_transition_sites(
    machine_info: &MachineInfo,
) -> Result<Vec<TransitionSite>, TokenStream> {
    let Some(file_path) = machine_info.file_path.as_deref() else {
        if is_rust_analyzer() {
            return Ok(Vec::new());
        }

        let message = format!(
            "Internal error: missing source file path while collecting transition sites for machine `{}`.",
            machine_info.name
        );
        return Err(syn::Error::new(proc_macro2::Span::call_site(), message).to_compile_error());
    };

    let Some(analysis) = get_file_analysis(file_path) else {
        if is_rust_analyzer() {
            return Ok(Vec::new());
        }

        let message = format!(
            "Internal error: could not analyze source file `{}` while collecting transition sites for machine `{}`.",
            file_path, machine_info.name
        );
        return Err(syn::Error::new(proc_macro2::Span::call_site(), message).to_compile_error());
    };

    let mut sites = Vec::new();
    for entry in &analysis.impls {
        if !entry.attrs.iter().any(|attr| attr == "transition") {
            continue;
        }

        let Some(module_path) = module_path_for_line(file_path, entry.line_number) else {
            continue;
        };
        if module_path != machine_info.module_path.as_ref() {
            continue;
        }

        let Ok(parsed) = parse_transition_impl(&entry.item) else {
            continue;
        };
        if parsed.machine_name != machine_info.name {
            continue;
        }

        for function in parsed.functions {
            let Ok(target_states) = function.return_states() else {
                continue;
            };
            sites.push(TransitionSite {
                method_name: function.name.to_string(),
                source_state: parsed.source_state.clone(),
                target_states,
            });
        }
    }

    Ok(sites)
}

pub(crate) fn to_pascal_case_identifier(value: &str) -> String {
    let mut result = String::new();
    for segment in value.trim_start_matches("r#").split('_') {
        if segment.is_empty() {
            continue;
        }

        let mut chars = segment.chars();
        if let Some(first) = chars.next() {
            for upper in first.to_uppercase() {
                result.push(upper);
            }
            result.extend(chars);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::to_pascal_case_identifier;

    #[test]
    fn pascal_case_identifier_handles_snake_case_and_raw_prefixes() {
        assert_eq!(to_pascal_case_identifier("validate"), "Validate");
        assert_eq!(to_pascal_case_identifier("start_review"), "StartReview");
        assert_eq!(to_pascal_case_identifier("r#await"), "Await");
    }
}
