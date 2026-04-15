//! `#[machine]` subsystem: validate machine structs, store registry facts, and emit machine surfaces.

mod emission;
mod generics;
mod introspection;
mod metadata;
mod registry;
mod validation;

use crate::diagnostics::DiagnosticMessage;
use proc_macro2::TokenStream;
use std::marker::PhantomData;
use syn::ItemStruct;

pub(crate) use emission::transition_support_module_ident;
pub use emission::generate_machine_impls;
pub(crate) use generics::{
    builder_generics, extra_generics, extra_type_arguments_tokens, generic_argument_tokens,
    machine_type_with_state,
};
pub(crate) use introspection::{
    to_shouty_snake_identifier, transition_presentation_slice_ident, transition_slice_ident,
};
pub(crate) use metadata::is_rust_analyzer;
pub use metadata::{MachineInfo, MachinePath};
pub(crate) use metadata::ParsedMachineInfo;
pub use registry::{
    LoadedMachineLookupFailure, format_loaded_machine_candidates,
    lookup_loaded_machine_in_module, lookup_unique_loaded_machine_by_name, store_machine_struct,
};
pub use validation::{invalid_machine_target_error, validate_machine_struct};

pub fn expand_machine(input: ItemStruct) -> TokenStream {
    MachineExpansionBuilder::<ParsedMachinePhase>::parse(input)
        .and_then(MachineExpansionBuilder::<ParsedMachinePhase>::validate)
        .map(MachineExpansionBuilder::<ValidatedMachinePhase>::register)
        .map(MachineExpansionBuilder::<RegisteredMachinePhase>::emit)
        .unwrap_or_else(|err| err)
}

struct ParsedMachinePhase;
struct ValidatedMachinePhase;
struct RegisteredMachinePhase;

struct MachineExpansionBuilder<State> {
    input: ItemStruct,
    machine_info: Option<MachineInfo>,
    _state: PhantomData<State>,
}

impl MachineExpansionBuilder<ParsedMachinePhase> {
    fn parse(input: ItemStruct) -> Result<Self, TokenStream> {
        let machine_info =
            MachineInfo::from_item_struct(&input).map_err(|err| err.to_compile_error())?;
        Ok(Self {
            input,
            machine_info: Some(machine_info),
            _state: PhantomData,
        })
    }

    fn validate(self) -> Result<MachineExpansionBuilder<ValidatedMachinePhase>, TokenStream> {
        let machine_info = self.machine_info.as_ref().ok_or_else(|| {
            syn::Error::new_spanned(
                &self.input.ident,
                DiagnosticMessage::new(format!(
                    "internal Statum error: parsed `#[machine]` pipeline for `{}` reached validation without machine metadata.",
                    self.input.ident
                ))
                .render(),
            )
            .to_compile_error()
        })?;

        if let Some(error) = validate_machine_struct(&self.input, machine_info) {
            return Err(error);
        }

        Ok(MachineExpansionBuilder {
            input: self.input,
            machine_info: self.machine_info,
            _state: PhantomData,
        })
    }
}

impl MachineExpansionBuilder<ValidatedMachinePhase> {
    fn register(self) -> MachineExpansionBuilder<RegisteredMachinePhase> {
        if let Some(machine_info) = self.machine_info.as_ref() {
            store_machine_struct(machine_info);
        }

        MachineExpansionBuilder {
            input: self.input,
            machine_info: self.machine_info,
            _state: PhantomData,
        }
    }
}

impl MachineExpansionBuilder<RegisteredMachinePhase> {
    fn emit(self) -> TokenStream {
        match self.machine_info {
            Some(machine_info) => generate_machine_impls(&machine_info, &self.input),
            None => syn::Error::new_spanned(
                &self.input.ident,
                DiagnosticMessage::new(format!(
                    "internal Statum error: registered `#[machine]` pipeline for `{}` reached emission without machine metadata.",
                    self.input.ident
                ))
                .render(),
            )
            .to_compile_error(),
        }
    }
}
