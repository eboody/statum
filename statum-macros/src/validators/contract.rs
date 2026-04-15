use std::collections::HashMap;

use quote::format_ident;
use syn::{Generics, Ident, ImplItemFn, Path, Type};

use crate::contracts::{ResolvedMachineRef, StateEnumContract, ValidatorContract};
use crate::VariantInfo;

use super::resolution::ValidatorMachineAttr;

pub(super) struct VariantSpec {
    pub(super) variant_name: String,
    pub(super) has_state_data: bool,
    pub(super) expected_ok_type: Type,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ValidatorReturnKind {
    Plain,
    Diagnostic,
}

pub(super) struct ValidatorMethodContract {
    pub(super) validator_fn: Ident,
    pub(super) variant_name: String,
    pub(super) has_state_data: bool,
    pub(super) return_kind: ValidatorReturnKind,
    pub(super) is_async: bool,
}

pub(super) struct ValidatorPlan {
    pub(super) methods: Vec<ValidatorMethodContract>,
    pub(super) has_async: bool,
}

pub(super) struct IntoMachineBuilderContext<'a> {
    pub(super) builder_ident: &'a Ident,
    pub(super) struct_ident: &'a Type,
    pub(super) machine_generics: &'a Generics,
    pub(super) machine_state_ty: &'a proc_macro2::TokenStream,
    pub(super) field_names: &'a [Ident],
    pub(super) field_types: &'a [Type],
    pub(super) validator_checks: &'a [proc_macro2::TokenStream],
    pub(super) validator_report_checks: &'a [proc_macro2::TokenStream],
    pub(super) async_token: &'a proc_macro2::TokenStream,
    pub(super) machine_vis: &'a syn::Visibility,
}

pub(super) fn build_validator_contract(
    machine_attr: &ValidatorMachineAttr,
    parsed_machine: crate::machine::ParsedMachineInfo,
    parsed_fields: &[(Ident, Type)],
    state_enum_info: crate::EnumInfo,
    persisted_type_display: &str,
) -> ValidatorContract {
    let field_names = parsed_fields
        .iter()
        .map(|(ident, _)| ident.clone())
        .collect::<Vec<_>>();
    let field_types = parsed_fields
        .iter()
        .map(|(_, ty)| ty.clone())
        .collect::<Vec<_>>();
    let machine_ident = machine_attr.machine_ident.clone();
    let machine_module_path =
        machine_support_module_path(&machine_attr.machine_path, &machine_attr.machine_name);

    ValidatorContract {
        resolved_machine: ResolvedMachineRef::new(
            machine_attr.machine_name.clone(),
            parsed_machine,
            machine_ident,
            machine_attr.machine_path.clone(),
            machine_module_path,
            field_names,
            field_types,
        ),
        state_enum: StateEnumContract::from(state_enum_info),
        persisted_type_display: persisted_type_display.to_string(),
        machine_attr_display: machine_attr.attr_display.clone(),
    }
}

pub(super) fn machine_support_module_path(machine_path: &Path, machine_name: &str) -> Path {
    let mut support_path = machine_path.clone();
    if let Some(last_segment) = support_path.segments.last_mut() {
        last_segment.ident = format_ident!("{}", crate::to_snake_case(machine_name));
    }
    support_path
}

pub(super) fn machine_scoped_item_path(machine_path: &Path, item_ident: &Ident) -> Path {
    let mut scoped_path = machine_path.clone();
    if let Some(last_segment) = scoped_path.segments.last_mut() {
        last_segment.ident = item_ident.clone();
    }
    scoped_path
}

pub(super) fn qualify_machine_field_types(
    parsed_fields: &[(Ident, Type)],
    machine_path: &Path,
) -> Vec<(Ident, Type)> {
    parsed_fields
        .iter()
        .map(|(ident, field_ty)| {
            (
                ident.clone(),
                qualify_machine_scoped_type(field_ty, machine_path),
            )
        })
        .collect()
}

fn qualify_machine_scoped_type(field_ty: &Type, machine_path: &Path) -> Type {
    let Type::Path(type_path) = field_ty else {
        return field_ty.clone();
    };
    if type_path.qself.is_some()
        || type_path.path.leading_colon.is_some()
        || type_path.path.segments.len() != 1
    {
        return field_ty.clone();
    }

    let Some(segment) = type_path.path.segments.last() else {
        return field_ty.clone();
    };
    let mut qualified = machine_scoped_item_path(machine_path, &segment.ident);
    if let Some(last_segment) = qualified.segments.last_mut() {
        last_segment.arguments = segment.arguments.clone();
    }

    syn::parse_quote!(#qualified)
}

pub(super) fn build_variant_lookup(
    variants: &[VariantInfo],
) -> Result<(Vec<VariantSpec>, HashMap<String, usize>), proc_macro2::TokenStream> {
    let mut specs = Vec::with_capacity(variants.len());
    let mut variant_by_name = HashMap::with_capacity(variants.len() * 2);

    for variant in variants {
        let state_data_type = variant.parse_data_type()?;
        specs.push(VariantSpec {
            variant_name: variant.name.clone(),
            has_state_data: state_data_type.is_some(),
            expected_ok_type: state_data_type.unwrap_or_else(|| syn::parse_quote!(())),
        });
        let idx = specs.len() - 1;
        variant_by_name.insert(variant.name.clone(), idx);
        variant_by_name.insert(crate::to_snake_case(&variant.name), idx);
    }

    Ok((specs, variant_by_name))
}

pub(super) fn build_validator_method_contract(
    func: &ImplItemFn,
    spec: &VariantSpec,
    return_kind: ValidatorReturnKind,
) -> ValidatorMethodContract {
    ValidatorMethodContract {
        validator_fn: func.sig.ident.clone(),
        variant_name: spec.variant_name.clone(),
        has_state_data: spec.has_state_data,
        return_kind,
        is_async: func.sig.asyncness.is_some(),
    }
}
