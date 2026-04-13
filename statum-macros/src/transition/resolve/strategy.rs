use std::collections::HashSet;

use syn::Type;

use super::alias::{AliasResolutionContext, expand_source_type_alias};
use super::shape::{
    SupportedWrapper, extract_first_generic_type_ref, extract_generic_type_refs,
    extract_machine_state_from_segment, machine_segment_matching_target, supported_wrapper,
    type_path,
};

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn parse_machine_and_state(ty: &Type, target_type: &Type) -> Option<(String, String)> {
    parse_primary_machine_and_state_in_context(ty, target_type, None)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn parse_machine_and_state_in_context(
    ty: &Type,
    target_type: &Type,
    context: Option<&AliasResolutionContext>,
) -> Option<(String, String)> {
    parse_primary_machine_and_state_with_alias_policy(ty, target_type, context, true)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn parse_primary_machine_and_state(
    ty: &Type,
    target_type: &Type,
) -> Option<(String, String)> {
    parse_primary_machine_and_state_with_alias_policy(ty, target_type, None, true)
}

fn parse_primary_machine_and_state_in_context(
    ty: &Type,
    target_type: &Type,
    context: Option<&AliasResolutionContext>,
) -> Option<(String, String)> {
    parse_primary_machine_and_state_with_alias_policy(ty, target_type, context, true)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn parse_primary_machine_and_state_strict(
    ty: &Type,
    target_type: &Type,
) -> Option<(String, String)> {
    parse_primary_machine_and_state_with_alias_policy(ty, target_type, None, false)
}

fn parse_primary_machine_and_state_with_alias_policy(
    ty: &Type,
    target_type: &Type,
    context: Option<&AliasResolutionContext>,
    allow_source_aliases: bool,
) -> Option<(String, String)> {
    let mut visited = HashSet::new();
    parse_primary_machine_and_state_inner(
        ty,
        target_type,
        context,
        allow_source_aliases,
        &mut visited,
    )
}

fn parse_primary_machine_and_state_inner(
    ty: &Type,
    target_type: &Type,
    context: Option<&AliasResolutionContext>,
    allow_source_aliases: bool,
    visited: &mut HashSet<String>,
) -> Option<(String, String)> {
    let type_path = type_path(ty)?;
    let segment = type_path.path.segments.last()?;

    if machine_segment_matching_target(&type_path.path, target_type).is_some() {
        return extract_machine_state_from_segment(segment)
            .map(|(machine, state, _)| (machine, state));
    }

    if allow_source_aliases
        && let Some((expanded, alias_context, visit_key)) =
            expand_source_type_alias(ty, context, visited)
    {
        let result = parse_primary_machine_and_state_inner(
            &expanded,
            target_type,
            Some(&alias_context),
            allow_source_aliases,
            visited,
        );
        visited.remove(&visit_key);
        return result;
    }

    match supported_wrapper(&type_path.path)? {
        SupportedWrapper::Option => extract_first_generic_type_ref(&segment.arguments).and_then(
            |inner| {
                parse_primary_machine_and_state_inner(
                    inner,
                    target_type,
                    context,
                    allow_source_aliases,
                    visited,
                )
            },
        ),
        SupportedWrapper::Result | SupportedWrapper::Branch => {
            extract_first_generic_type_ref(&segment.arguments).and_then(|inner| {
                parse_primary_machine_and_state_inner(
                    inner,
                    target_type,
                    context,
                    allow_source_aliases,
                    visited,
                )
            })
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn collect_machine_and_states(
    ty: &Type,
    target_type: &Type,
) -> Vec<(String, String)> {
    collect_machine_and_states_with_alias_policy(ty, target_type, None, true)
}

pub(crate) fn collect_machine_and_states_in_context(
    ty: &Type,
    target_type: &Type,
    context: Option<&AliasResolutionContext>,
) -> Vec<(String, String)> {
    collect_machine_and_states_with_alias_policy(ty, target_type, context, true)
}

pub(crate) fn collect_machine_and_states_strict(
    ty: &Type,
    target_type: &Type,
) -> Vec<(String, String)> {
    collect_machine_and_states_with_alias_policy(ty, target_type, None, false)
}

fn collect_machine_and_states_with_alias_policy(
    ty: &Type,
    target_type: &Type,
    context: Option<&AliasResolutionContext>,
    allow_source_aliases: bool,
) -> Vec<(String, String)> {
    let mut targets = Vec::new();
    let mut visited = HashSet::new();
    collect_machine_targets_inner(
        ty,
        target_type,
        context,
        allow_source_aliases,
        &mut visited,
        &mut targets,
    );
    targets
}

fn collect_machine_targets_inner(
    ty: &Type,
    target_type: &Type,
    context: Option<&AliasResolutionContext>,
    allow_source_aliases: bool,
    visited: &mut HashSet<String>,
    targets: &mut Vec<(String, String)>,
) {
    let Some(type_path) = type_path(ty) else {
        return;
    };
    let Some(segment) = type_path.path.segments.last() else {
        return;
    };

    if machine_segment_matching_target(&type_path.path, target_type).is_some() {
        if let Some((machine, state, _)) = extract_machine_state_from_segment(segment) {
            push_unique_target(targets, machine, state);
        }
        return;
    }

    if allow_source_aliases
        && let Some((expanded, alias_context, visit_key)) =
            expand_source_type_alias(ty, context, visited)
    {
        collect_machine_targets_inner(
            &expanded,
            target_type,
            Some(&alias_context),
            allow_source_aliases,
            visited,
            targets,
        );
        visited.remove(&visit_key);
        return;
    }

    match supported_wrapper(&type_path.path) {
        Some(SupportedWrapper::Option) => {
            if let Some(inner) = extract_first_generic_type_ref(&segment.arguments) {
                collect_machine_targets_inner(
                    inner,
                    target_type,
                    context,
                    allow_source_aliases,
                    visited,
                    targets,
                );
            }
        }
        Some(SupportedWrapper::Result | SupportedWrapper::Branch) => {
            if let Some(types) = extract_generic_type_refs(&segment.arguments) {
                for inner in types {
                    collect_machine_targets_inner(
                        inner,
                        target_type,
                        context,
                        allow_source_aliases,
                        visited,
                        targets,
                    );
                }
            }
        }
        None => {}
    }
}

fn push_unique_target(targets: &mut Vec<(String, String)>, machine: String, state: String) {
    if !targets.iter().any(|(existing_machine, existing_state)| {
        existing_machine == &machine && existing_state == &state
    }) {
        targets.push((machine, state));
    }
}
