//! Shared semantic contracts passed between parsing, resolution, diagnostics, and emission.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Generics, Ident, Path, Type};

use crate::machine::{MachineInfo, ParsedMachineInfo, extra_type_arguments_tokens};
use crate::{EnumInfo, VariantInfo};

#[derive(Clone)]
pub(crate) struct StateEnumContract {
    pub(crate) enum_info: EnumInfo,
    pub(crate) name: String,
    pub(crate) variants: Vec<VariantInfo>,
}

impl From<EnumInfo> for StateEnumContract {
    fn from(enum_info: EnumInfo) -> Self {
        Self {
            name: enum_info.name.clone(),
            variants: enum_info.variants.clone(),
            enum_info,
        }
    }
}

#[derive(Clone)]
pub(crate) struct ResolvedMachineRef {
    #[allow(dead_code)]
    pub(crate) machine_info: MachineInfo,
    pub(crate) parsed_machine: ParsedMachineInfo,
    pub(crate) machine_ident: Ident,
    pub(crate) machine_name: String,
    pub(crate) machine_path: Path,
    pub(crate) machine_module_path: Path,
    pub(crate) state_enum_name: String,
    pub(crate) field_names: Vec<Ident>,
    pub(crate) field_types: Vec<Type>,
    pub(crate) machine_state_ty: TokenStream,
}

impl ResolvedMachineRef {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        machine_info: MachineInfo,
        parsed_machine: ParsedMachineInfo,
        machine_ident: Ident,
        machine_path: Path,
        machine_module_path: Path,
        state_enum_name: String,
        field_names: Vec<Ident>,
        field_types: Vec<Type>,
    ) -> Self {
        let machine_extra_ty_args = extra_type_arguments_tokens(&parsed_machine.generics);
        let machine_state_ty = quote! { #machine_module_path::SomeState #machine_extra_ty_args };

        Self {
            machine_name: machine_info.name.clone(),
            machine_info,
            parsed_machine,
            machine_ident,
            machine_path,
            machine_module_path,
            state_enum_name,
            field_names,
            field_types,
            machine_state_ty,
        }
    }

    pub(crate) fn machine_generics(&self) -> &Generics {
        &self.parsed_machine.generics
    }
}

#[allow(dead_code)]
#[derive(Clone)]
pub(crate) struct TransitionContract {
    pub(crate) machine_name: String,
    pub(crate) source_state_name: String,
    pub(crate) primary_next_state: String,
    pub(crate) next_states: Vec<String>,
    pub(crate) strict_introspection: bool,
    pub(crate) written_return_type: Option<String>,
}

impl TransitionContract {
    pub(crate) fn all_next_states(&self) -> Vec<&str> {
        let mut states = vec![self.primary_next_state.as_str()];
        states.extend(
            self.next_states
                .iter()
                .map(String::as_str)
                .filter(|state| *state != self.primary_next_state),
        );
        states
    }
}

#[derive(Clone)]
pub(crate) struct ValidatorContract {
    pub(crate) resolved_machine: ResolvedMachineRef,
    pub(crate) state_enum: StateEnumContract,
    pub(crate) persisted_type_display: String,
    pub(crate) machine_attr_display: String,
}
