mod diagnostics;
mod emit;
mod parse;
mod resolve;

pub use diagnostics::{
    ambiguous_transition_machine_error, ambiguous_transition_machine_fallback_error,
    missing_transition_machine_error,
};
pub use emit::generate_transition_impl;
pub use parse::parse_transition_impl;

use self::diagnostics::{
    compile_error_at, invalid_transition_method_state_error, invalid_transition_state_error,
    machine_return_signature,
};
use self::parse::TransitionImpl;
use crate::diagnostics::DiagnosticMessage;
use crate::MachineInfo;
use proc_macro2::TokenStream;
use syn::spanned::Spanned;

pub fn validate_transition_functions(
    tr_impl: &TransitionImpl,
    machine_info: &MachineInfo,
) -> Option<TokenStream> {
    if tr_impl.functions.is_empty() {
        let message = DiagnosticMessage::new(format!(
            "`#[transition]` impl for `{}<{}>` must contain at least one transition method.",
            tr_impl.machine_name,
            tr_impl.source_state,
        ))
        .found(format!("`impl {}<{}> {{}}`", tr_impl.machine_name, tr_impl.source_state))
        .expected(format!(
            "`fn submit(self) -> {}` or a supported wrapper around that same machine path",
            machine_return_signature(&tr_impl.machine_name),
        ))
        .fix("add at least one method that consumes `self` and returns the next `#[machine]` state.")
        .render();
        return Some(compile_error_at(tr_impl.target_type.span(), &message));
    }

    let state_enum_info = match machine_info.get_matching_state_enum() {
        Ok(enum_info) => enum_info,
        Err(err) => return Some(err),
    };

    if state_enum_info
        .get_variant_from_name(&tr_impl.source_state)
        .is_none()
    {
        return Some(invalid_transition_state_error(
            tr_impl.source_state_span,
            &tr_impl.machine_name,
            &tr_impl.source_state,
            &state_enum_info,
            "source",
        ));
    }

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
            return Some(compile_error_at(func.span, &message));
        }

        let return_state = match func.return_state(&tr_impl.target_type) {
            Ok(state) => state,
            Err(err) => return Some(err),
        };
        if state_enum_info.get_variant_from_name(&return_state).is_none() {
            return Some(invalid_transition_method_state_error(
                func,
                &tr_impl.machine_name,
                &return_state,
                &state_enum_info,
            ));
        }

        let return_states = match func.return_states(&tr_impl.target_type) {
            Ok(states) => states,
            Err(err) => return Some(err),
        };
        for return_state in return_states {
            if state_enum_info.get_variant_from_name(&return_state).is_none() {
                return Some(invalid_transition_method_state_error(
                    func,
                    &tr_impl.machine_name,
                    &return_state,
                    &state_enum_info,
                ));
            }
        }
    }

    None
}
