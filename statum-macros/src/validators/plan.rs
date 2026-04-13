use syn::ItemImpl;

use crate::VariantInfo;

use super::contract::{CollectValidatorContext, build_variant_lookup};
use super::emission::{
    ValidatorCheckContext, generate_validator_check, generate_validator_report_check,
};
use super::signatures::{
    ValidatorDiagnosticContext, validate_validator_return_type, validate_validator_signature,
    validator_state_name_from_ident,
};

pub(super) fn collect_validator_checks(
    item_impl: &ItemImpl,
    variants: &[VariantInfo],
    context: &CollectValidatorContext<'_>,
) -> Result<
    (
        Vec<proc_macro2::TokenStream>,
        Vec<proc_macro2::TokenStream>,
        bool,
    ),
    proc_macro2::TokenStream,
> {
    let mut checks = Vec::new();
    let mut report_checks = Vec::new();
    let mut has_async = false;
    let receiver = quote::quote! { __statum_persisted };
    let (variant_specs, variant_by_name) = build_variant_lookup(variants)?;
    let emission_context = ValidatorCheckContext {
        machine_path: context.machine_path,
        machine_module_path: context.machine_module_path,
        machine_generics: context.machine_generics,
        field_names: context.field_names,
        receiver: &receiver,
    };

    for item in &item_impl.items {
        let syn::ImplItem::Fn(func) = item else {
            continue;
        };

        let Some(state_name) = validator_state_name_from_ident(&func.sig.ident) else {
            continue;
        };
        let Some(spec_idx) = variant_by_name.get(&state_name) else {
            continue;
        };
        let spec = &variant_specs[*spec_idx];
        let diagnostic_context = ValidatorDiagnosticContext {
            persisted_type_display: context.persisted_type_display,
            machine_name: context.machine_name,
            state_enum_name: context.state_enum_name,
            variant_name: &spec.variant_name,
            machine_fields: context.field_names,
            expected_ok_type: &spec.expected_ok_type,
        };
        validate_validator_signature(func, &diagnostic_context)?;
        let return_kind =
            validate_validator_return_type(func, &spec.expected_ok_type, &diagnostic_context)?;

        if func.sig.asyncness.is_some() {
            has_async = true;
        }
        checks.push(generate_validator_check(
            &emission_context,
            &spec.variant_name,
            spec.has_state_data,
            func.sig.asyncness.is_some(),
        ));
        report_checks.push(generate_validator_report_check(
            &emission_context,
            &spec.variant_name,
            spec.has_state_data,
            return_kind,
            func.sig.asyncness.is_some(),
        ));
    }

    Ok((checks, report_checks, has_async))
}
