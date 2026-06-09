use proc_macro2::TokenStream;
use syn::spanned::Spanned;

use super::ValidatedTransitionMethod;
use super::contract::build_transition_contract;
use super::diagnostics::{
    compile_error_at, invalid_transition_method_state_error, invalid_transition_state_error,
    machine_return_signature,
};
use super::parse::TransitionImpl;
use crate::MachineInfo;
use crate::diagnostics::DiagnosticMessage;

pub(super) fn validate_transition_functions(
    tr_impl: &TransitionImpl,
    machine_info: &MachineInfo,
) -> Result<Vec<ValidatedTransitionMethod>, TokenStream> {
    if tr_impl.functions.is_empty() {
        let message = DiagnosticMessage::new(format!(
            "`#[transition]` impl for `{}<{}>` must contain at least one transition method.",
            tr_impl.machine_name, tr_impl.source_state,
        ))
        .found(format!(
            "`impl {}<{}> {{}}`",
            tr_impl.machine_name, tr_impl.source_state
        ))
        .expected(format!(
            "`fn submit(self) -> {}` or a supported wrapper around that same machine path",
            machine_return_signature(&tr_impl.machine_name),
        ))
        .fix(
            "add at least one method that consumes `self` and returns the next `#[machine]` state.",
        )
        .render();
        return Err(compile_error_at(tr_impl.target_type.span(), &message));
    }

    let state_enum_info = machine_info.get_matching_state_enum()?;

    if state_enum_info
        .get_variant_from_name(&tr_impl.source_state)
        .is_none()
    {
        return Err(invalid_transition_state_error(
            tr_impl.source_state_span,
            &tr_impl.machine_name,
            &tr_impl.source_state,
            &state_enum_info,
            "source",
        ));
    }

    let mut validated_methods = Vec::with_capacity(tr_impl.functions.len());
    for func in &tr_impl.functions {
        if !func.has_receiver {
            let message = DiagnosticMessage::new(format!(
                "`#[transition]` method `{}<{}>::{}` must take `self` or `mut self` as its receiver.",
                tr_impl.machine_name,
                tr_impl.source_state,
                func.name,
            ))
            .found(format!("`fn {}(...)`", func.name))
            .expected(format!("`fn {}(self) -> {}`", func.name, machine_return_signature(&tr_impl.machine_name)))
            .fix("change the method receiver to `self` or `mut self`.".to_string())
            .render();
            return Err(compile_error_at(func.span, &message));
        }

        let contract = build_transition_contract(func, &tr_impl.target_type)?;
        for return_state in contract.all_next_states() {
            if state_enum_info
                .get_variant_from_name(return_state)
                .is_none()
            {
                return Err(invalid_transition_method_state_error(
                    func,
                    &tr_impl.machine_name,
                    return_state,
                    &state_enum_info,
                ));
            }
        }
        validated_methods.push(ValidatedTransitionMethod {
            function: func.clone(),
            contract,
        });
    }

    Ok(validated_methods)
}
