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
use std::marker::PhantomData;
use syn::spanned::Spanned;
use syn::ItemImpl;

pub(crate) struct ValidatedTransitionMethod {
    pub(crate) function: parse::TransitionFn,
    pub(crate) contract: TransitionContract,
}

pub fn expand_transition(input: ItemImpl) -> TokenStream {
    TransitionExpansionBuilder::<ParsedTransitionPhase>::parse(input)
        .and_then(TransitionExpansionBuilder::<ParsedTransitionPhase>::resolve_machine)
        .and_then(TransitionExpansionBuilder::<ResolvedTransitionPhase>::validate)
        .map(TransitionExpansionBuilder::<ValidatedTransitionPhase>::emit)
        .unwrap_or_else(|err| err)
}

struct ParsedTransitionPhase;
struct ResolvedTransitionPhase;
struct ValidatedTransitionPhase;

struct TransitionExpansionBuilder<State> {
    input: ItemImpl,
    tr_impl: TransitionImpl,
    module_path: String,
    machine_info: Option<MachineInfo>,
    validated_methods: Vec<ValidatedTransitionMethod>,
    _state: PhantomData<State>,
}

impl TransitionExpansionBuilder<ParsedTransitionPhase> {
    fn parse(input: ItemImpl) -> Result<Self, TokenStream> {
        let tr_impl = parse_transition_impl(&input)?;
        let module_path = resolved_current_module_path(tr_impl.machine_span, "#[transition]")?;
        Ok(Self {
            input,
            tr_impl,
            module_path,
            machine_info: None,
            validated_methods: Vec::new(),
            _state: PhantomData,
        })
    }

    fn resolve_machine(self) -> Result<TransitionExpansionBuilder<ResolvedTransitionPhase>, TokenStream> {
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

        Ok(TransitionExpansionBuilder {
            input: self.input,
            tr_impl: self.tr_impl,
            module_path: self.module_path,
            machine_info: Some(machine_info),
            validated_methods: Vec::new(),
            _state: PhantomData,
        })
    }
}

impl TransitionExpansionBuilder<ResolvedTransitionPhase> {
    fn validate(self) -> Result<TransitionExpansionBuilder<ValidatedTransitionPhase>, TokenStream> {
        let machine_info = self.machine_info.ok_or_else(|| {
            compile_error_at(
                self.tr_impl.target_type.span(),
                &format!(
                    "internal Statum error: resolved `#[transition]` pipeline for `{}` reached validation without machine metadata.",
                    self.tr_impl.machine_name,
                ),
            )
        })?;
        let validated_methods = validate_transition_functions(&self.tr_impl, &machine_info)?;
        Ok(TransitionExpansionBuilder {
            input: self.input,
            tr_impl: self.tr_impl,
            module_path: self.module_path,
            machine_info: Some(machine_info),
            validated_methods,
            _state: PhantomData,
        })
    }
}

impl TransitionExpansionBuilder<ValidatedTransitionPhase> {
    fn emit(self) -> TokenStream {
        let machine_info = match self.machine_info {
            Some(machine_info) => machine_info,
            None => {
                return compile_error_at(
                    self.tr_impl.target_type.span(),
                    &format!(
                        "internal Statum error: validated `#[transition]` pipeline for `{}` reached emission without machine metadata.",
                        self.tr_impl.machine_name,
                    ),
                );
            }
        };
        generate_transition_impl(
            &self.input,
            &self.tr_impl,
            &machine_info,
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
