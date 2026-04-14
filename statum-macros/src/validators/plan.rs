use syn::ItemImpl;

use crate::contracts::ValidatorContract;

use super::contract::{ValidatorPlan, build_variant_lookup};
use super::signatures::{
    ValidatorDiagnosticContext, build_validator_method_contract, validator_state_name_from_ident,
};

pub(super) fn collect_validator_plan(
    item_impl: &ItemImpl,
    contract: &ValidatorContract,
) -> Result<ValidatorPlan, proc_macro2::TokenStream> {
    let mut methods = Vec::new();
    let mut has_async = false;
    let (variant_specs, variant_by_name) = build_variant_lookup(&contract.state_enum.variants)?;

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
            persisted_type_display: &contract.persisted_type_display,
            machine_name: &contract.resolved_machine.machine_name,
            state_enum_name: &contract.state_enum.name,
            variant_name: &spec.variant_name,
            machine_fields: &contract.resolved_machine.field_names,
            expected_ok_type: &spec.expected_ok_type,
        };
        let method_contract = build_validator_method_contract(func, spec, &diagnostic_context)?;

        if method_contract.is_async {
            has_async = true;
        }
        methods.push(method_contract);
    }

    Ok(ValidatorPlan { methods, has_async })
}
