use syn::Type;

use crate::diagnostics::compact_display;

use super::super::parse::{TransitionFn, TransitionIntrospectAttr};
use super::targets::{
    observed_return_shape, primary_branch_display, strict_introspect_return_suggestion,
};

pub(crate) struct InvalidReturnTypeFacts {
    pub(crate) written_return_type: String,
    pub(crate) expected_signature: String,
    pub(crate) fix: String,
    pub(crate) primary_branch: Option<String>,
    pub(crate) observed_machine_branches: Vec<String>,
    pub(crate) strict_help: Option<String>,
}

pub(crate) fn describe_invalid_return_type(
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
        .unwrap_or_else(|| {
            format!(
                "`fn {}(self) -> {}<NextState>`",
                func.name, func.machine_name
            )
        });
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
        primary_branch: observed
            .as_ref()
            .and_then(|shape| shape.primary_branch.clone()),
        observed_machine_branches: observed
            .map(|shape| shape.secondary_machine_branches)
            .unwrap_or_default(),
        strict_help,
    }
}

pub(crate) struct IntrospectReturnMismatchFacts {
    pub(crate) expected: String,
    pub(crate) fix: String,
    pub(crate) written_primary_branch: Option<String>,
    pub(crate) annotated_primary_branch: Option<String>,
    pub(crate) observed_machine_branches: Vec<String>,
}

pub(crate) fn describe_mismatched_introspect_return(
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
        .unwrap_or_else(|| {
            format!("an annotation describing the same legal targets as `{actual_return}`")
        });
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
        written_primary_branch: observed
            .as_ref()
            .and_then(|shape| shape.primary_branch.clone()),
        annotated_primary_branch: annotation_primary_branch,
        observed_machine_branches: observed
            .map(|shape| shape.secondary_machine_branches)
            .unwrap_or_default(),
    }
}
