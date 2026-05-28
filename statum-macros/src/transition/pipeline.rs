use proc_macro2::TokenStream;
use std::marker::PhantomData;
use syn::spanned::Spanned;
use syn::ItemImpl;

use super::diagnostics::{
    ambiguous_transition_machine_error, ambiguous_transition_machine_fallback_error,
    compile_error_at, missing_transition_machine_error,
};
use super::emit::generate_transition_impl;
use super::parse::{parse_transition_impl, TransitionImpl};
use super::resolve::missing_transition_machine_context;
use super::validation::validate_transition_functions;
use crate::{
    LoadedMachineLookupFailure, MachineInfo, MachinePath, lookup_loaded_machine_in_module,
    lookup_unique_loaded_machine_by_name, resolved_current_module_path,
};

pub(super) fn expand_transition(input: ItemImpl) -> TokenStream {
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
    validated_methods: Vec<super::ValidatedTransitionMethod>,
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
                        let context = missing_transition_machine_context(
                            &self.tr_impl.machine_name,
                            &self.module_path,
                        );
                        return Err(
                            missing_transition_machine_error(
                                &self.tr_impl.machine_name,
                                &self.module_path,
                                &context,
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
