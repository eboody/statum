use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Ident, Path, PathArguments};

use crate::diagnostics::{DiagnosticMessage, compile_error_at, compact_display};
use crate::source::{
    ItemCandidate, ItemKind, candidates_in_module, current_source_info, format_candidates,
    module_path_from_file_with_root, module_root_from_file, plain_item_line_in_module,
    same_named_candidates_elsewhere,
};

use crate::{
    EnumInfo, LoadedMachineLookupFailure, LoadedStateLookupFailure, MachineInfo, MachinePath,
    StateModulePath, format_loaded_machine_candidates, format_loaded_state_candidates,
    lookup_loaded_machine_in_module, lookup_loaded_state_enum, lookup_loaded_state_enum_by_name,
    lookup_unique_loaded_machine_by_name,
};

pub(super) struct ValidatorMachineAttr {
    pub(super) machine_path: Path,
    pub(super) machine_ident: Ident,
    pub(super) machine_name: String,
    pub(super) machine_module_path: String,
    pub(super) attr_display: String,
    pub(super) path_kind: ValidatorMachinePathKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ValidatorMachinePathKind {
    BareCurrentModule,
    Anchored,
    RelativeMultiSegment,
}

pub(super) fn resolve_validator_machine_attr(
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

pub(super) fn resolve_machine_metadata(
    current_module_path: &str,
    machine_attr: &ValidatorMachineAttr,
) -> Result<MachineInfo, TokenStream> {
    let module_path = machine_attr.machine_module_path.as_str();
    let module_path_key: MachinePath = module_path.into();
    let machine_name = machine_attr.machine_name.as_str();
    lookup_loaded_machine_in_module(&module_path_key, machine_name).map_err(|failure| {
        let current_line = current_source_info().map(|(_, line)| line).unwrap_or_default();
        let available = available_machine_candidates_in_module(module_path);
        let suggested_machine_name = available
            .first()
            .map(|candidate| candidate.name.as_str())
            .unwrap_or(machine_name);
        let suggested_attr = preferred_machine_attr_suggestion(
            current_module_path,
            machine_name,
            Some(module_path),
            suggested_machine_name,
        )
        .unwrap_or_else(|| format!("crate::path::{suggested_machine_name}"));
        let available_line = if available.is_empty() {
            "No `#[machine]` items were found in this module.".to_string()
        } else {
            format!(
                "Available `#[machine]` items in this module: {}.",
                format_candidates(&available)
            )
        };
        let ordering_line = available
            .iter()
            .find(|candidate| {
                candidate.name == machine_name && candidate.line_number > current_line
            })
            .map(|candidate| {
                format!(
                    "Source scan found `#[machine]` item `{machine_name}` later in this module on line {}. If that item is active for this build, move it above this `#[validators]` impl because Statum resolves these relationships in expansion order.",
                    candidate.line_number
                )
            })
            .map(|line| format!("{line}\n"))
            .unwrap_or_default();
        let elsewhere_line = same_named_machine_candidates_elsewhere(machine_name, module_path)
            .map(|candidates| {
                format!(
                    "Same-named `#[machine]` items elsewhere in this file: {}.",
                    format_candidates(&candidates)
                )
            })
            .unwrap_or_else(|| "No same-named `#[machine]` items were found in other modules of this file.".to_string());
        let missing_attr_line = plain_struct_line_in_module(module_path, machine_name).map(|line| {
            format!(
                "A struct named `{machine_name}` exists on line {line}, but it is not annotated with `#[machine]`."
            )
        });
        let relative_path_line = unresolved_relative_validator_path_line(
            machine_attr,
            &suggested_attr,
        )
        .map(|line| format!("{line}\n"))
        .unwrap_or_default();
        let authority_line = match failure {
            LoadedMachineLookupFailure::NotFound => {
                "Statum only resolves `#[machine]` items that have already expanded before this `#[validators]` impl.".to_string()
            }
            LoadedMachineLookupFailure::Ambiguous(candidates) => format!(
                "Loaded `#[machine]` candidates were ambiguous: {}.",
                format_loaded_machine_candidates(&candidates)
            ),
        };
        let message = DiagnosticMessage::new(format!(
            "`#[validators({})]` could not resolve a matching `#[machine]` in module `{module_path}`.",
            machine_attr.attr_display,
        ))
        .found(format!("`#[validators({})]`", machine_attr.attr_display))
        .expected(format!("`#[validators({suggested_attr})]`"))
        .fix("point `#[validators(...)]` at the Statum machine type declared in that module and declare that `#[machine]` item before this validators impl.".to_string())
        .reason(authority_line)
        .assumption(relative_path_line.trim_end().to_string())
        .note(ordering_line.trim_end().to_string())
        .note(
            missing_attr_line
                .unwrap_or_else(|| "No plain struct with that name was found in this module either.".to_string()),
        )
        .candidates(elsewhere_line)
        .candidates(available_line)
        .help(format!(
            "Correct shape: `#[validators({suggested_attr})] impl PersistedRow {{ ... }}`."
        ));
        compile_error_at(proc_macro2::Span::call_site(), &message)
    })
}

pub(super) fn resolve_state_enum_info(
    machine_metadata: &MachineInfo,
) -> Result<EnumInfo, TokenStream> {
    let module_path = machine_metadata.module_path.as_ref();
    let state_path_key: StateModulePath = module_path.into();
    let machine_name = machine_metadata.name.clone();
    let expected_state_name = machine_metadata.state_generic_name.as_deref();
    let state_enum_info = match expected_state_name {
        Some(expected_name) => lookup_loaded_state_enum_by_name(&state_path_key, expected_name),
        None => lookup_loaded_state_enum(&state_path_key),
    };
    state_enum_info.map_err(|failure| {
        let current_line = current_source_info().map(|(_, line)| line).unwrap_or_default();
        let available = available_state_candidates_in_module(module_path);
        let available_line = if available.is_empty() {
            "No `#[state]` enums were found in this module.".to_string()
        } else {
            format!(
                "Available `#[state]` enums in this module: {}.",
                format_candidates(&available)
            )
        };
        let elsewhere_line = expected_state_name
            .and_then(|name| same_named_state_candidates_elsewhere(name, module_path))
            .map(|candidates| {
                format!(
                    "Same-named `#[state]` enums elsewhere in this file: {}.",
                    format_candidates(&candidates)
                )
            })
            .unwrap_or_else(|| "No same-named `#[state]` enums were found in other modules of this file.".to_string());
        let expected_line = expected_state_name
            .map(|name| {
                format!(
                    "Machine `{machine_name}` expects its first generic parameter to name `#[state]` enum `{name}`."
                )
            })
            .unwrap_or_else(|| {
                format!(
                    "Machine `{machine_name}` did not expose a resolvable first generic parameter for its `#[state]` enum."
                )
            });
        let ordering_line = expected_state_name.and_then(|name| {
            available
                .iter()
                .find(|candidate| {
                    candidate.name == name && candidate.line_number > current_line
                })
                .map(|candidate| {
                    format!(
                        "Source scan found `#[state]` enum `{name}` later in this module on line {}. If that item is active for this build, move it above the machine and this `#[validators]` impl because Statum resolves these relationships in expansion order.",
                        candidate.line_number
                    )
                })
        });
        let ordering_line = ordering_line
            .map(|line| format!("{line}\n"))
            .unwrap_or_default();
        let missing_attr_line = expected_state_name.as_ref().and_then(|name| {
            plain_enum_line_in_module(module_path, name).map(|line| {
                format!("An enum named `{name}` exists on line {line}, but it is not annotated with `#[state]`.")
            })
        });
        let authority_line = match failure {
            LoadedStateLookupFailure::NotFound => {
                "Statum only resolves `#[state]` enums that have already expanded before this `#[validators]` impl.".to_string()
            }
            LoadedStateLookupFailure::Ambiguous(candidates) => format!(
                "Loaded `#[state]` candidates were ambiguous: {}.",
                format_loaded_state_candidates(&candidates)
            ),
        };
        let message = DiagnosticMessage::new(format!(
            "machine `{machine_name}` could not resolve its `#[state]` enum in module `{module_path}` for this `#[validators]` impl."
        ))
        .expected(format!(
            "`struct {machine_name}<ExpectedState> {{ ... }}` where `ExpectedState` is a `#[state]` enum declared in `{module_path}`"
        ))
        .fix("make the machine's first generic name the right local `#[state]` enum and declare that enum before the machine and validators impl.".to_string())
        .reason(expected_line)
        .note(authority_line)
        .note(ordering_line.trim_end().to_string())
        .note(
            missing_attr_line
                .unwrap_or_else(|| "No plain enum with that expected name was found in this module either.".to_string()),
        )
        .candidates(elsewhere_line)
        .candidates(available_line);
        compile_error_at(proc_macro2::Span::call_site(), &message)
    })
}

fn validate_validator_machine_path(machine_path: &Path) -> Result<(), TokenStream> {
    let Some(last_segment) = machine_path.segments.last() else {
        let message = DiagnosticMessage::new(
            "`#[validators(...)]` requires a machine path.",
        )
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
        let suggestion = preferred_machine_attr_suggestion(current_module_path, machine_name, None, machine_name)
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

fn unresolved_relative_validator_path_line(
    machine_attr: &ValidatorMachineAttr,
    suggested_attr: &str,
) -> Option<String> {
    if machine_attr.path_kind != ValidatorMachinePathKind::RelativeMultiSegment {
        return None;
    }

    Some(format!(
        "Path note: Statum interpreted `{}` as the local child-module path `self::{}`.\nImported aliases and re-exports are not supported in `#[validators(...)]` path resolution.\nIf you meant that local module, declare the `#[machine]` there or spell it `#[validators(self::{})]` for clarity. If you meant a different module, anchor the real path, for example `#[validators({suggested_attr})]`.",
        machine_attr.attr_display,
        machine_attr.attr_display,
        machine_attr.attr_display,
    ))
}

fn preferred_machine_attr_suggestion(
    current_module_path: &str,
    machine_name: &str,
    fallback_module_path: Option<&str>,
    fallback_machine_name: &str,
) -> Option<String> {
    if let Ok(machine_info) = lookup_unique_loaded_machine_by_name(machine_name) {
        return Some(machine_attr_display_for_module(
            current_module_path,
            machine_info.module_path.as_ref(),
            &machine_info.name,
        ));
    }

    fallback_module_path.map(|module_path| {
        machine_attr_display_for_module(current_module_path, module_path, fallback_machine_name)
    })
}

fn machine_attr_display_for_module(
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

fn available_machine_candidates_in_module(module_path: &str) -> Vec<ItemCandidate> {
    let Some((file_path, _)) = current_source_info() else {
        return Vec::new();
    };
    candidates_in_module(&file_path, module_path, ItemKind::Struct, Some("machine"))
}

fn available_state_candidates_in_module(module_path: &str) -> Vec<ItemCandidate> {
    let Some((file_path, _)) = current_source_info() else {
        return Vec::new();
    };
    candidates_in_module(&file_path, module_path, ItemKind::Enum, Some("state"))
}

fn same_named_machine_candidates_elsewhere(
    machine_name: &str,
    module_path: &str,
) -> Option<Vec<ItemCandidate>> {
    let (file_path, _) = current_source_info()?;
    let candidates = same_named_candidates_elsewhere(
        &file_path,
        module_path,
        ItemKind::Struct,
        machine_name,
        Some("machine"),
    );
    (!candidates.is_empty()).then_some(candidates)
}

fn same_named_state_candidates_elsewhere(
    state_name: &str,
    module_path: &str,
) -> Option<Vec<ItemCandidate>> {
    let (file_path, _) = current_source_info()?;
    let candidates = same_named_candidates_elsewhere(
        &file_path,
        module_path,
        ItemKind::Enum,
        state_name,
        Some("state"),
    );
    (!candidates.is_empty()).then_some(candidates)
}

fn plain_struct_line_in_module(module_path: &str, struct_name: &str) -> Option<usize> {
    let (file_path, _) = current_source_info()?;
    plain_item_line_in_module(
        &file_path,
        module_path,
        ItemKind::Struct,
        struct_name,
        Some("machine"),
    )
}

fn plain_enum_line_in_module(module_path: &str, enum_name: &str) -> Option<usize> {
    let (file_path, _) = current_source_info()?;
    plain_item_line_in_module(
        &file_path,
        module_path,
        ItemKind::Enum,
        enum_name,
        Some("state"),
    )
}
