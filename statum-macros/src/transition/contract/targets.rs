use proc_macro2::TokenStream;
use std::collections::HashSet;
use syn::{GenericArgument, PathArguments, Type};

use crate::contracts::TransitionContract;
use crate::diagnostics::compact_display;

use super::super::diagnostics::{
    invalid_introspect_return_error, invalid_return_type_error, mismatched_introspect_return_error,
};
use super::super::parse::TransitionFn;
use super::super::resolve::{
    AliasResolutionContext, SourceAliasResolver, SupportedWrapper,
    collect_machine_and_states_in_context, collect_machine_and_states_strict,
    expand_source_type_alias, extract_first_generic_type_ref, extract_generic_type_refs,
    machine_segment_matching_target, parse_machine_and_state_in_context,
    parse_primary_machine_and_state_strict, supported_wrapper, type_path,
};

pub(crate) fn build_transition_contract(
    func: &TransitionFn,
    target_type: &syn::Type,
) -> Result<TransitionContract, TokenStream> {
    let return_contract = validate_transition_return_contract(func, target_type)?;
    Ok(TransitionContract {
        primary_next_state: return_contract.primary_next_state,
        next_states: return_contract.next_states,
    })
}

pub(super) struct ObservedReturnShape {
    pub(super) primary_branch: Option<String>,
    pub(super) secondary_machine_branches: Vec<String>,
    pub(super) wrapper: Option<SupportedWrapper>,
    pub(super) canonical_state: Option<String>,
}

impl ObservedReturnShape {
    pub(super) fn canonical_machine_target(&self, machine_name: &str) -> String {
        match self.canonical_state.as_deref() {
            Some(state) => format!("{machine_name}<{state}>"),
            None => format!("{machine_name}<NextState>"),
        }
    }

    pub(super) fn canonical_annotation(&self, machine_name: &str) -> String {
        let machine_target = self.canonical_machine_target(machine_name);
        match self.wrapper {
            Some(SupportedWrapper::Option) => {
                format!("::core::option::Option<{machine_target}>")
            }
            Some(SupportedWrapper::Result) => {
                format!("::core::result::Result<{machine_target}, E>")
            }
            Some(SupportedWrapper::Branch) => {
                format!("::statum::Branch<{machine_target}, OtherBranch>")
            }
            None => machine_target,
        }
    }

    pub(super) fn canonical_wrapped_signature(
        &self,
        func_name: &syn::Ident,
        machine_name: &str,
    ) -> String {
        format!(
            "`fn {func_name}(self) -> {}`",
            self.canonical_annotation(machine_name)
        )
    }

    pub(super) fn fix_message(&self, func_name: &syn::Ident, machine_name: &str) -> String {
        let machine_target = self.canonical_machine_target(machine_name);
        match self.wrapper {
            Some(SupportedWrapper::Option)
            | Some(SupportedWrapper::Result)
            | Some(SupportedWrapper::Branch) => format!(
                "move `{machine_target}` into the primary branch, for example with {}, or return `{machine_target}` directly if you do not need the wrapper.",
                self.canonical_wrapped_signature(func_name, machine_name)
            ),
            None => format!("return `{machine_target}` directly."),
        }
    }
}

struct ValidatedTransitionReturnContract {
    primary_next_state: String,
    next_states: Vec<String>,
}

struct ResolvedTransitionTargets {
    primary_next_state: String,
    next_states: Vec<String>,
}

struct ObservedTransitionTargets {
    primary_next_state: Option<String>,
    next_states: Vec<String>,
}

fn validate_transition_return_contract(
    func: &TransitionFn,
    target_type: &Type,
) -> Result<ValidatedTransitionReturnContract, TokenStream> {
    let Some(written_return_type) = func.return_type.as_ref() else {
        return Err(invalid_return_type_error(
            func,
            target_type,
            "missing return type",
        ));
    };

    let strict_introspection =
        crate::strict_introspection_enabled() || func.introspection.is_some();
    if let Some(introspection) = func.introspection.as_ref() {
        let introspection_targets = resolve_transition_targets_strict(&introspection.return_type, target_type)
            .ok_or_else(|| {
                invalid_introspect_return_error(
                    introspection,
                    func,
                    "expected a direct machine path or a supported `Option`, `Result`, or `statum::Branch` wrapper around that machine path",
                )
            })?;

        let written_targets = resolve_transition_targets(
            written_return_type,
            target_type,
            false,
            func.return_type_span,
        )
        .ok_or_else(|| {
            invalid_return_type_error(
                func,
                target_type,
                "even with `#[introspect(return = ...)]`, the written return type must still resolve to the impl target machine path or a supported wrapper around it",
            )
        })?;
        if written_targets.primary_next_state != introspection_targets.primary_next_state
            || written_targets.next_states != introspection_targets.next_states
        {
            return Err(mismatched_introspect_return_error(
                introspection,
                func,
                written_return_type,
                target_type,
            ));
        }

        return Ok(ValidatedTransitionReturnContract {
            primary_next_state: introspection_targets.primary_next_state,
            next_states: introspection_targets.next_states,
        });
    }

    let reason = if strict_introspection {
        "expected the impl target machine path directly, or that same machine path wrapped in a supported `Option`, `Result`, or `Branch` shape; aliases require an explicit `#[introspect(return = ...)]` annotation in strict mode"
    } else {
        "expected the impl target machine path directly, a source-backed type alias that expands to it, or that same machine path wrapped in a supported `Option`, `Result`, or `Branch` shape"
    };
    let targets = resolve_transition_targets(
        written_return_type,
        target_type,
        strict_introspection,
        func.return_type_span,
    )
    .ok_or_else(|| invalid_return_type_error(func, target_type, reason))?;

    Ok(ValidatedTransitionReturnContract {
        primary_next_state: targets.primary_next_state,
        next_states: targets.next_states,
    })
}

fn resolve_transition_targets(
    ty: &Type,
    target_type: &Type,
    strict: bool,
    return_type_span: Option<proc_macro2::Span>,
) -> Option<ResolvedTransitionTargets> {
    if strict {
        return resolve_transition_targets_strict(ty, target_type);
    }

    SourceAliasResolver::new(return_type_span)
        .find_map(|context| resolve_transition_targets_in_context(ty, target_type, context))
}

fn resolve_transition_targets_strict(
    ty: &Type,
    target_type: &Type,
) -> Option<ResolvedTransitionTargets> {
    let observed = observe_transition_targets_strict(ty, target_type);
    let primary_next_state = observed.primary_next_state?;
    let next_states = observed.next_states;
    (!next_states.is_empty()).then_some(ResolvedTransitionTargets {
        primary_next_state,
        next_states,
    })
}

fn resolve_transition_targets_in_context(
    ty: &Type,
    target_type: &Type,
    context: Option<&super::super::resolve::AliasResolutionContext>,
) -> Option<ResolvedTransitionTargets> {
    let (_, primary_next_state) = parse_machine_and_state_in_context(ty, target_type, context)?;
    let next_states = collect_machine_and_states_in_context(ty, target_type, context)
        .into_iter()
        .map(|(_, state)| state)
        .collect::<Vec<_>>();
    (!next_states.is_empty()).then_some(ResolvedTransitionTargets {
        primary_next_state,
        next_states,
    })
}

fn observe_transition_targets_strict(ty: &Type, target_type: &Type) -> ObservedTransitionTargets {
    ObservedTransitionTargets {
        primary_next_state: parse_primary_machine_and_state_strict(ty, target_type)
            .map(|(_, state)| state),
        next_states: collect_machine_and_states_strict(ty, target_type)
            .into_iter()
            .map(|(_, state)| state)
            .collect::<Vec<_>>(),
    }
}

pub(super) fn strict_introspect_return_suggestion(
    func: &TransitionFn,
    target_type: &Type,
) -> Option<String> {
    let return_type = func.return_type.as_ref()?;
    SourceAliasResolver::new(func.return_type_span)
        .find_map(|context| {
            strict_diagnostic_expanded_return_type(return_type, target_type, context)
        })
        .map(|expanded| compact_display(&expanded))
}

fn strict_diagnostic_expanded_return_type(
    ty: &Type,
    target_type: &Type,
    context: Option<&AliasResolutionContext>,
) -> Option<Type> {
    let mut visited = HashSet::new();
    strict_diagnostic_expanded_return_type_inner(ty, target_type, context, &mut visited)
}

fn strict_diagnostic_expanded_return_type_inner(
    ty: &Type,
    target_type: &Type,
    context: Option<&AliasResolutionContext>,
    visited: &mut HashSet<String>,
) -> Option<Type> {
    let type_path = type_path(ty)?;

    if machine_segment_matching_target(&type_path.path, target_type).is_some() {
        return Some(ty.clone());
    }

    if let Some(expanded_alias) = expand_source_type_alias(ty, context, visited) {
        let (expanded, alias_context, visit_key) = expanded_alias.into_parts();
        let result = strict_diagnostic_expanded_return_type_inner(
            &expanded,
            target_type,
            Some(&alias_context),
            visited,
        )
        .or_else(|| {
            parse_primary_machine_and_state_strict(&expanded, target_type)
                .is_some()
                .then_some(expanded.clone())
        });
        visited.remove(&visit_key);
        return result;
    }

    let segment = type_path.path.segments.last()?;
    supported_wrapper(&type_path.path)?;

    let original_types = extract_generic_type_refs(&segment.arguments)?;
    let mut expanded_ty = ty.clone();
    let Type::Path(expanded_type_path) = &mut expanded_ty else {
        return None;
    };
    let expanded_segment = expanded_type_path.path.segments.last_mut()?;
    let PathArguments::AngleBracketed(args) = &mut expanded_segment.arguments else {
        return None;
    };

    let mut expanded_any = false;
    let mut type_index = 0usize;
    for arg in &mut args.args {
        let GenericArgument::Type(inner_ty) = arg else {
            continue;
        };
        let original_inner = original_types.get(type_index)?;
        if let Some(expanded_inner) = strict_diagnostic_expanded_return_type_inner(
            original_inner,
            target_type,
            context,
            visited,
        ) {
            *inner_ty = expanded_inner;
            expanded_any = true;
        }
        type_index += 1;
    }

    if expanded_any && parse_primary_machine_and_state_strict(&expanded_ty, target_type).is_some() {
        Some(expanded_ty)
    } else {
        None
    }
}

pub(super) fn observed_return_shape(
    func: &TransitionFn,
    target_type: &Type,
) -> Option<ObservedReturnShape> {
    let return_type = func.return_type.as_ref()?;
    let wrapper = raw_wrapper_kind(return_type);
    let primary_branch = primary_branch_display(return_type);
    let mut machine_branches = resolved_machine_branches(func, target_type);
    let canonical_state = parse_primary_machine_and_state_strict(return_type, target_type)
        .map(|(_, state)| state)
        .or_else(|| {
            machine_branches
                .first()
                .map(|branch| state_name_from_machine_target(branch).to_string())
        });
    if let Some(state) = canonical_state.as_deref() {
        let canonical_machine = format!("{}<{state}>", func.machine_name);
        machine_branches.retain(|branch| branch != &canonical_machine);
    }

    Some(ObservedReturnShape {
        primary_branch,
        secondary_machine_branches: machine_branches,
        wrapper,
        canonical_state,
    })
}

fn resolved_machine_branches(func: &TransitionFn, target_type: &Type) -> Vec<String> {
    let Some(return_type) = func.return_type.as_ref() else {
        return Vec::new();
    };
    let uses_strict_resolution =
        crate::strict_introspection_enabled() || func.introspection.is_some();
    let targets = if uses_strict_resolution {
        collect_machine_and_states_strict(return_type, target_type)
    } else {
        SourceAliasResolver::new(func.return_type_span)
            .find_map(|context| {
                let states =
                    collect_machine_and_states_in_context(return_type, target_type, context);
                (!states.is_empty()).then_some(states)
            })
            .unwrap_or_default()
    };

    targets
        .into_iter()
        .map(|(machine, state)| format!("{machine}<{state}>"))
        .collect()
}

fn raw_wrapper_kind(ty: &Type) -> Option<SupportedWrapper> {
    let type_path = type_path(ty)?;
    supported_wrapper(&type_path.path)
}

pub(super) fn primary_branch_display(ty: &Type) -> Option<String> {
    let type_path = type_path(ty)?;
    let segment = type_path.path.segments.last()?;
    match supported_wrapper(&type_path.path) {
        Some(_) => extract_first_generic_type_ref(&segment.arguments).map(compact_display),
        None => Some(compact_display(ty)),
    }
}

fn state_name_from_machine_target(machine_target: &str) -> &str {
    machine_target
        .split_once('<')
        .and_then(|(_, state)| state.strip_suffix('>'))
        .unwrap_or("NextState")
}
