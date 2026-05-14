use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Ident, Path, PathArguments};

use crate::diagnostics::{DiagnosticMessage, compact_display, compile_error_at};
use crate::source::{current_source_info, module_path_from_file_with_root, module_root_from_file};

use super::lookup::preferred_machine_attr_suggestion;

pub(crate) struct ValidatorMachineAttr {
    pub(crate) machine_path: Path,
    pub(crate) machine_ident: Ident,
    pub(crate) machine_name: String,
    pub(crate) machine_module_path: String,
    pub(crate) attr_display: String,
    pub(crate) path_kind: ValidatorMachinePathKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ValidatorMachinePathKind {
    BareCurrentModule,
    Anchored,
    RelativeMultiSegment,
}

pub(crate) fn resolve_validator_machine_attr(
    current_module_path: &str,
    machine_path: &Path,
) -> Result<ValidatorMachineAttr, TokenStream> {
    validate_validator_machine_path(machine_path)?;

    let machine_ident = machine_path
        .segments
        .last()
        .map(|segment| segment.ident.clone())
        .expect("validated machine path has a last segment");
    let machine_name = machine_ident.to_string();
    let attr_display = path_display(machine_path);
    let path_kind = validator_machine_path_kind(machine_path);
    let machine_module_path = resolve_validator_machine_module_path(
        current_module_path,
        machine_path,
        &machine_name,
    )?;

    Ok(ValidatorMachineAttr {
        machine_path: machine_path.clone(),
        machine_ident,
        machine_name,
        machine_module_path,
        attr_display,
        path_kind,
    })
}

fn validate_validator_machine_path(machine_path: &Path) -> Result<(), TokenStream> {
    let Some(last_segment) = machine_path.segments.last() else {
        let message = DiagnosticMessage::new("`#[validators(...)]` requires a machine path.")
            .expected("`Machine` or `crate::flow::Machine`")
            .fix("write `#[validators(Machine)]` for the current module or an anchored path like `#[validators(crate::flow::Machine)]`.");
        return Err(compile_error_at(proc_macro2::Span::call_site(), &message));
    };

    if machine_path.leading_colon.is_some() {
        let message = DiagnosticMessage::new(
            "`#[validators(...)]` does not accept leading-`::` paths.",
        )
        .found(format!("`{}`", compact_display(machine_path)))
        .expected("`Machine`, `self::flow::Machine`, `super::flow::Machine`, or `crate::flow::Machine`")
        .fix("drop the leading `::` and anchor the path with `crate::`, `self::`, or `super::` when needed.");
        return Err(compile_error_at(proc_macro2::Span::call_site(), &message));
    }

    if machine_path
        .segments
        .iter()
        .any(|segment| !matches!(segment.arguments, PathArguments::None))
    {
        let message = DiagnosticMessage::new(
            "`#[validators(...)]` expects a machine type path without generic arguments.",
        )
        .found(format!("`{}`", compact_display(machine_path)))
        .expected("`Machine` or `crate::flow::Machine`")
        .fix("remove generic arguments from the attribute path. Statum reads the machine type itself, not a concrete instantiation.");
        return Err(compile_error_at(proc_macro2::Span::call_site(), &message));
    }

    let reserved = last_segment.ident.to_string();
    if reserved == "crate" || reserved == "self" || reserved == "super" {
        let message = DiagnosticMessage::new(
            "`#[validators(...)]` must end with the Statum machine type name.",
        )
        .found(format!("`{}`", compact_display(machine_path)))
        .expected("`Machine` or `crate::flow::Machine`")
        .fix("end the path with the machine type itself, for example `#[validators(crate::flow::Machine)]`.");
        return Err(compile_error_at(proc_macro2::Span::call_site(), &message));
    }

    Ok(())
}

fn resolve_validator_machine_module_path(
    current_module_path: &str,
    machine_path: &Path,
    machine_name: &str,
) -> Result<String, TokenStream> {
    let segments = machine_path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    let module_segments = &segments[..segments.len().saturating_sub(1)];
    if module_segments.is_empty() {
        return Ok(current_module_path.to_owned());
    }

    let first = module_segments[0].as_str();
    let relative_is_ambiguous = !module_segments.is_empty()
        && first != "crate"
        && first != "self"
        && first != "super";
    if crate::strict_introspection_enabled() && relative_is_ambiguous {
        let suggestion = preferred_machine_attr_suggestion(
            current_module_path,
            machine_name,
            None,
            machine_name,
        )
        .unwrap_or_else(|| format!("crate::path::{machine_name}"));
        let written_path = path_display(machine_path);
        let message = DiagnosticMessage::new(format!(
            "`#[validators({written_path})]` is not accepted in strict introspection mode."
        ))
        .found(format!("`#[validators({written_path})]`"))
        .expected(format!("`#[validators({suggestion})]`"))
        .fix("use a direct machine path rooted at `crate::`, `self::`, or `super::`.".to_string())
        .reason(format!(
            "relative multi-segment paths like `{written_path}` can name either module paths or imported aliases, and strict mode only accepts locally readable machine bindings."
        ));
        return Err(compile_error_at(proc_macro2::Span::call_site(), &message));
    }

    let mut index = 0usize;
    let mut base = match first {
        "crate" => {
            index = 1;
            source_observation_root_module()
        }
        "self" => {
            index = 1;
            current_module_path.to_owned()
        }
        "super" => {
            let mut module = current_module_path.to_owned();
            while module_segments
                .get(index)
                .is_some_and(|segment| segment == "super")
            {
                module = parent_module_path(&module).ok_or_else(|| {
                    let message = "Error: `#[validators(super::...)]` climbed past the crate root.\nFix: use `crate::...` for an absolute machine path.";
                    quote! { compile_error!(#message); }
                })?;
                index += 1;
            }
            module
        }
        _ => current_module_path.to_owned(),
    };

    for segment in &module_segments[index..] {
        base = child_module_path(&base, segment);
    }

    Ok(base)
}

fn validator_machine_path_kind(machine_path: &Path) -> ValidatorMachinePathKind {
    let module_segment_count = machine_path.segments.len().saturating_sub(1);
    if module_segment_count == 0 {
        return ValidatorMachinePathKind::BareCurrentModule;
    }

    let first = machine_path
        .segments
        .first()
        .map(|segment| segment.ident.to_string())
        .expect("validated machine path has a first segment");
    match first.as_str() {
        "crate" | "self" | "super" => ValidatorMachinePathKind::Anchored,
        _ => ValidatorMachinePathKind::RelativeMultiSegment,
    }
}

pub(crate) fn unresolved_relative_validator_path_line(
    machine_attr: &ValidatorMachineAttr,
    suggested_attr: &str,
) -> Option<String> {
    if machine_attr.path_kind != ValidatorMachinePathKind::RelativeMultiSegment {
        return None;
    }

    Some(format!(
        "Path note: Statum interpreted `{}` as the local child-module path `self::{}`.\nImported aliases and re-exports are not supported in `#[validators(...)]` path resolution.\nIf you meant that local module, declare the `#[machine]` there or spell it `#[validators(self::{})]` for clarity. If you meant a different module, anchor the real path, for example `#[validators({suggested_attr})]`.",
        machine_attr.attr_display, machine_attr.attr_display, machine_attr.attr_display,
    ))
}

pub(crate) fn machine_attr_display_for_module(
    current_module_path: &str,
    module_path: &str,
    machine_name: &str,
) -> String {
    if module_path == current_module_path {
        machine_name.to_owned()
    } else if module_path == "crate" {
        format!("crate::{machine_name}")
    } else if module_path.starts_with("crate::") {
        format!("{module_path}::{machine_name}")
    } else {
        format!("crate::{module_path}::{machine_name}")
    }
}

fn parent_module_path(module_path: &str) -> Option<String> {
    if module_path == "crate" {
        return None;
    }

    module_path
        .rsplit_once("::")
        .map(|(parent, _)| parent.to_owned())
        .or_else(|| Some("crate".to_owned()))
}

fn child_module_path(base: &str, child: &str) -> String {
    if base == "crate" {
        child.to_owned()
    } else {
        format!("{base}::{child}")
    }
}

fn path_display(path: &Path) -> String {
    path.to_token_stream().to_string().replace(" :: ", "::")
}

fn source_observation_root_module() -> String {
    let Some((file_path, _)) = current_source_info() else {
        return "crate".to_owned();
    };

    if let Some(crate_root) = crate::crate_root_for_file(&file_path) {
        let src_root = std::path::PathBuf::from(crate_root).join("src");
        if std::path::PathBuf::from(&file_path).starts_with(&src_root) {
            return "crate".to_owned();
        }
    }

    let module_root = module_root_from_file(&file_path);
    module_path_from_file_with_root(&file_path, &module_root)
}
