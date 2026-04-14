use proc_macro2::TokenStream;
use syn::Type;

use crate::contracts::TransitionContract;
use crate::diagnostics::compact_display;

use super::diagnostics::{
    invalid_introspect_return_error, invalid_return_type_error,
    mismatched_introspect_return_error,
};
use super::resolve::{
    candidate_alias_resolution_contexts, collect_machine_and_states_in_context,
    collect_machine_and_states_strict, parse_machine_and_state_in_context,
    parse_primary_machine_and_state_strict,
};
use super::parse::TransitionFn;

pub(super) fn build_transition_contract(
    func: &TransitionFn,
    target_type: &syn::Type,
) -> Result<TransitionContract, TokenStream> {
    let return_contract = validate_transition_return_contract(func, target_type)?;
    Ok(TransitionContract {
        machine_name: func.machine_name.clone(),
        source_state_name: func.source_state.clone(),
        primary_next_state: return_contract.primary_next_state,
        next_states: return_contract.next_states,
        strict_introspection: return_contract.strict_introspection,
        written_return_type: func.return_type.as_ref().map(compact_display),
    })
}

struct ValidatedTransitionReturnContract {
    primary_next_state: String,
    next_states: Vec<String>,
    strict_introspection: bool,
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

        let written_targets = observe_transition_targets_strict(written_return_type, target_type);
        if !written_targets.next_states.is_empty()
            && (written_targets.primary_next_state.as_deref()
                != Some(introspection_targets.primary_next_state.as_str())
                || written_targets.next_states != introspection_targets.next_states)
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
            strict_introspection: true,
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
        strict_introspection,
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

    let contexts = candidate_alias_resolution_contexts(return_type_span);
    contexts
        .iter()
        .find_map(|context| resolve_transition_targets_in_context(ty, target_type, Some(context)))
        .or_else(|| resolve_transition_targets_in_context(ty, target_type, None))
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
