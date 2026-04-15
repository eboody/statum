//! Transition macro pipeline: parse impls, resolve return-shape contracts, then emit code.

mod contract;
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
use self::contract::build_transition_contract;
use self::parse::TransitionImpl;
use crate::contracts::TransitionContract;
use crate::diagnostics::DiagnosticMessage;
use crate::{
    LoadedMachineLookupFailure, MachineInfo, MachinePath, lookup_loaded_machine_in_module,
    lookup_unique_loaded_machine_by_name, resolved_current_module_path,
};
use proc_macro2::TokenStream;
use syn::spanned::Spanned;
use syn::ItemImpl;

pub(crate) struct ValidatedTransitionMethod {
    pub(crate) function: parse::TransitionFn,
    pub(crate) contract: TransitionContract,
}

pub fn expand_transition(input: ItemImpl) -> TokenStream {
    ParsedTransitionExpansion::parse(input)
        .and_then(ParsedTransitionExpansion::resolve_machine)
        .and_then(ResolvedTransitionExpansion::validate)
        .map(ValidatedTransitionExpansion::emit)
        .unwrap_or_else(|err| err)
}

struct ParsedTransitionExpansion {
    input: ItemImpl,
    tr_impl: TransitionImpl,
    module_path: String,
}

impl ParsedTransitionExpansion {
    fn parse(input: ItemImpl) -> Result<Self, TokenStream> {
        let tr_impl = parse_transition_impl(&input)?;
        let module_path = resolved_current_module_path(tr_impl.machine_span, "#[transition]")?;
        Ok(Self {
            input,
            tr_impl,
            module_path,
        })
    }

    fn resolve_machine(self) -> Result<ResolvedTransitionExpansion, TokenStream> {
        let machine_path: MachinePath = self.module_path.clone().into();
        let machine_info = match lookup_loaded_machine_in_module(&machine_path, &self.tr_impl.machine_name)
        {
            Ok(info) => info,
            Err(LoadedMachineLookupFailure::Ambiguous(candidates)) => {
                return Err(
                    ambiguous_transition_machine_error(
                        &self.tr_impl.machine_name,
                        &self.module_path,
                        &candidates,
                        self.tr_impl.machine_span,
                    ),
                );
            }
            Err(LoadedMachineLookupFailure::NotFound) => {
                match lookup_unique_loaded_machine_by_name(&self.tr_impl.machine_name) {
                    Ok(info) => info,
                    Err(LoadedMachineLookupFailure::Ambiguous(candidates)) => {
                        return Err(
                            ambiguous_transition_machine_fallback_error(
                                &self.tr_impl.machine_name,
                                &self.module_path,
                                &candidates,
                                self.tr_impl.machine_span,
                            ),
                        );
                    }
                    Err(LoadedMachineLookupFailure::NotFound) => {
                        return Err(
                            missing_transition_machine_error(
                                &self.tr_impl.machine_name,
                                &self.module_path,
                                self.tr_impl.machine_span,
                            ),
                        );
                    }
                }
            }
        };

        Ok(ResolvedTransitionExpansion {
            input: self.input,
            tr_impl: self.tr_impl,
            machine_info,
        })
    }
}

struct ResolvedTransitionExpansion {
    input: ItemImpl,
    tr_impl: TransitionImpl,
    machine_info: MachineInfo,
}

impl ResolvedTransitionExpansion {
    fn validate(self) -> Result<ValidatedTransitionExpansion, TokenStream> {
        let validated_methods = validate_transition_functions(&self.tr_impl, &self.machine_info)?;
        Ok(ValidatedTransitionExpansion {
            input: self.input,
            tr_impl: self.tr_impl,
            machine_info: self.machine_info,
            validated_methods,
        })
    }
}

struct ValidatedTransitionExpansion {
    input: ItemImpl,
    tr_impl: TransitionImpl,
    machine_info: MachineInfo,
    validated_methods: Vec<ValidatedTransitionMethod>,
}

impl ValidatedTransitionExpansion {
    fn emit(self) -> TokenStream {
        generate_transition_impl(
            &self.input,
            &self.tr_impl,
            &self.machine_info,
            &self.validated_methods,
        )
    }
}

pub fn validate_transition_functions(
    tr_impl: &TransitionImpl,
    machine_info: &MachineInfo,
) -> Result<Vec<ValidatedTransitionMethod>, TokenStream> {
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
            if state_enum_info.get_variant_from_name(return_state).is_none() {
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
