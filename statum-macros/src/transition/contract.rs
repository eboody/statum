use proc_macro2::TokenStream;
use syn::Type;

use crate::contracts::TransitionContract;
use crate::diagnostics::compact_display;

use super::diagnostics::{
    invalid_introspect_return_error, invalid_return_type_error,
    mismatched_introspect_return_error,
};
use super::resolve::{
    AliasResolutionContext, SourceAliasResolver, SupportedWrapper,
    collect_machine_and_states_in_context, collect_machine_and_states_strict,
    expand_source_type_alias, extract_first_generic_type_ref, extract_generic_type_refs,
    machine_segment_matching_target, parse_machine_and_state_in_context,
    parse_primary_machine_and_state_strict, supported_wrapper, type_path,
};
use super::parse::{TransitionFn, TransitionIntrospectAttr};
use std::collections::HashSet;
use syn::{GenericArgument, PathArguments};

pub(super) fn build_transition_contract(
    func: &TransitionFn,
    target_type: &syn::Type,
) -> Result<TransitionContract, TokenStream> {
    let return_contract = validate_transition_return_contract(func, target_type)?;
    Ok(TransitionContract {
        primary_next_state: return_contract.primary_next_state,
        next_states: return_contract.next_states,
    })
}

pub(super) struct InvalidReturnTypeFacts {
    pub(super) written_return_type: String,
    pub(super) expected_signature: String,
    pub(super) fix: String,
    pub(super) primary_branch: Option<String>,
    pub(super) observed_machine_branches: Vec<String>,
    pub(super) strict_help: Option<String>,
}

pub(super) fn describe_invalid_return_type(
    func: &TransitionFn,
    target_type: &Type,
) -> InvalidReturnTypeFacts {
    let written_return_type = func
        .return_type
        .as_ref()
        .map(compact_display)
        .unwrap_or_else(|| "<none>".to_string());
    let uses_strict_resolution =
        crate::strict_introspection_enabled() || func.introspection.is_some();
    let observed = observed_return_shape(func, target_type);
    let expected_signature = observed
        .as_ref()
        .map(|shape| shape.canonical_wrapped_signature(&func.name, &func.machine_name))
        .unwrap_or_else(|| format!("`fn {}(self) -> {}<NextState>`", func.name, func.machine_name));
    let fix = observed
        .as_ref()
        .map(|shape| shape.fix_message(&func.name, &func.machine_name))
        .unwrap_or_else(|| {
            format!(
                "return `{}<NextState>` directly, or wrap that same machine path in a supported `Option`, `Result`, or `statum::Branch` shape.",
                func.machine_name
            )
        });
    let strict_help = if uses_strict_resolution {
        Some(
            strict_introspect_return_suggestion(func, target_type)
                .map(|expanded| {
                    format!(
                        "add `#[introspect(return = {expanded})]` to this method, or rewrite the signature to use that direct type.\nSource-backed alias expansion is diagnostics-only in strict mode."
                    )
                })
                .unwrap_or_else(|| {
                    "add `#[introspect(return = Machine<NextState>)]` with a direct machine path and supported wrapper shape, or rewrite the signature to use that direct type.\nSource-backed alias expansion is diagnostics-only in strict mode.".to_string()
                }),
        )
    } else {
        None
    };

    InvalidReturnTypeFacts {
        written_return_type,
        expected_signature,
        fix,
        primary_branch: observed.as_ref().and_then(|shape| shape.primary_branch.clone()),
        observed_machine_branches: observed
            .map(|shape| shape.secondary_machine_branches)
            .unwrap_or_default(),
        strict_help,
    }
}

pub(super) struct IntrospectReturnMismatchFacts {
    pub(super) expected: String,
    pub(super) fix: String,
    pub(super) written_primary_branch: Option<String>,
    pub(super) annotated_primary_branch: Option<String>,
    pub(super) observed_machine_branches: Vec<String>,
}

pub(super) fn describe_mismatched_introspect_return(
    introspection: &TransitionIntrospectAttr,
    func: &TransitionFn,
    actual_return_type: &Type,
    target_type: &Type,
) -> IntrospectReturnMismatchFacts {
    let actual_return = compact_display(actual_return_type);
    let observed = observed_return_shape(func, target_type);
    let annotation_primary_branch = primary_branch_display(&introspection.return_type);
    let expected = observed
        .as_ref()
        .map(|shape| {
            format!(
                "`#[introspect(return = {})]` and {}",
                shape.canonical_annotation(&func.machine_name),
                shape.canonical_wrapped_signature(&func.name, &func.machine_name)
            )
        })
        .unwrap_or_else(|| format!("an annotation describing the same legal targets as `{actual_return}`"));
    let fix = observed
        .as_ref()
        .map(|shape| {
            format!(
                "make the written primary branch `{}` so it matches `#[introspect(return = {})]`, or rewrite the method to {}.",
                shape.canonical_machine_target(&func.machine_name),
                shape.canonical_annotation(&func.machine_name),
                shape.canonical_wrapped_signature(&func.name, &func.machine_name)
            )
        })
        .unwrap_or_else(|| "either remove the annotation or make it match the written signature.".to_string());

    IntrospectReturnMismatchFacts {
        expected,
        fix,
        written_primary_branch: observed.as_ref().and_then(|shape| shape.primary_branch.clone()),
        annotated_primary_branch: annotation_primary_branch,
        observed_machine_branches: observed
            .map(|shape| shape.secondary_machine_branches)
            .unwrap_or_default(),
    }
}

struct ValidatedTransitionReturnContract {
    primary_next_state: String,
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

    let strict_introspection = crate::strict_introspection_enabled() || func.introspection.is_some();
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

    let targets = resolve_transition_targets(
        written_return_type,
        target_type,
        strict_introspection,
        func.return_type_span,
    )
    .ok_or_else(|| {
        invalid_return_type_error(
            func,
            target_type,
            "expected the impl target machine path directly, a source-backed type alias that expands to it, or that same machine path wrapped in a supported `Option`, `Result`, or `Branch` shape",
        )
    })?;

    Ok(ValidatedTransitionReturnContract {
        primary_next_state: targets.primary_next_state,
        next_states: targets.next_states,
    })
}

struct ResolvedTransitionTargets {
    primary_next_state: String,
    next_states: Vec<String>,
}

struct ObservedTransitionTargets {
    primary_next_state: Option<String>,
    next_states: Vec<String>,
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
    context: Option<&super::resolve::AliasResolutionContext>,
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

fn observe_transition_targets_strict(
    ty: &Type,
    target_type: &Type,
) -> ObservedTransitionTargets {
    ObservedTransitionTargets {
        primary_next_state: parse_primary_machine_and_state_strict(ty, target_type)
            .map(|(_, state)| state),
        next_states: collect_machine_and_states_strict(ty, target_type)
            .into_iter()
            .map(|(_, state)| state)
            .collect::<Vec<_>>(),
    }
}

fn strict_introspect_return_suggestion(
    func: &TransitionFn,
    target_type: &Type,
) -> Option<String> {
    let return_type = func.return_type.as_ref()?;
    SourceAliasResolver::new(func.return_type_span)
        .find_map(|context| strict_diagnostic_expanded_return_type(return_type, target_type, context))
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

    if let Some(expanded_alias) = expand_source_type_alias(ty, context, visited)
    {
        let (expanded, alias_context, visit_key) = expanded_alias.into_parts();
        let result =
            strict_diagnostic_expanded_return_type_inner(&expanded, target_type, Some(&alias_context), visited)
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

struct ObservedReturnShape {
    primary_branch: Option<String>,
    secondary_machine_branches: Vec<String>,
    wrapper: Option<SupportedWrapper>,
    canonical_state: Option<String>,
}

impl ObservedReturnShape {
    fn canonical_machine_target(&self, machine_name: &str) -> String {
        match self.canonical_state.as_deref() {
            Some(state) => format!("{machine_name}<{state}>"),
            None => format!("{machine_name}<NextState>"),
        }
    }

    fn canonical_annotation(&self, machine_name: &str) -> String {
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

    fn canonical_wrapped_signature(&self, func_name: &syn::Ident, machine_name: &str) -> String {
        format!("`fn {func_name}(self) -> {}`", self.canonical_annotation(machine_name))
    }

    fn fix_message(&self, func_name: &syn::Ident, machine_name: &str) -> String {
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

fn observed_return_shape(func: &TransitionFn, target_type: &Type) -> Option<ObservedReturnShape> {
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

fn primary_branch_display(ty: &Type) -> Option<String> {
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
