use std::collections::HashSet;

use syn::ItemImpl;

use crate::contracts::ValidatorContract;
use crate::diagnostics::{DiagnosticMessage, compile_error_at};
use crate::to_snake_case;

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
    let mut existing = HashSet::new();
    let (variant_specs, variant_by_name) = build_variant_lookup(&contract.state_enum.variants)?;
    let valid_state_names = contract
        .state_enum
        .variants
        .iter()
        .map(|variant| to_snake_case(&variant.name))
        .collect::<HashSet<_>>();

    for item in &item_impl.items {
        let syn::ImplItem::Fn(func) = item else {
            continue;
        };

        let Some(state_name) = validator_state_name_from_ident(&func.sig.ident) else {
            continue;
        };
        existing.insert(state_name.clone());
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

    let unknown = existing
        .iter()
        .filter(|name| !valid_state_names.contains(*name))
        .map(|name| format!("is_{name}"))
        .collect::<Vec<_>>();
    if !unknown.is_empty() {
        let unknown_list = unknown.join(", ");
        let state_enum_name = &contract.state_enum.name;
        let valid_list = contract
            .state_enum
            .variants
            .iter()
            .map(|variant| format!("is_{}", to_snake_case(&variant.name)))
            .collect::<Vec<_>>()
            .join(", ");
        let message = DiagnosticMessage::new(format!(
            "`#[validators({})]` on `impl {}` defines methods that do not match any variant in `{state_enum_name}`.",
            contract.machine_attr_display,
            contract.persisted_type_display,
        ))
        .found(format!("unknown validator methods: `{unknown_list}`"))
        .expected(format!(
            "one `is_{{state}}` method per `{}` state: `{valid_list}`",
            contract.resolved_machine.machine_name
        ))
        .fix("rename or remove methods that do not correspond to a `#[state]` variant.".to_string());
        return Err(compile_error_at(proc_macro2::Span::call_site(), &message));
    }

    let missing = contract
        .state_enum
        .variants
        .iter()
        .map(|variant| to_snake_case(&variant.name))
        .filter(|name| !existing.contains(name))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        let missing_list = missing
            .iter()
            .map(|name| format!("is_{name}"))
            .collect::<Vec<_>>()
            .join(", ");
        let state_enum_name = &contract.state_enum.name;
        let message = DiagnosticMessage::new(format!(
            "`#[validators({})]` on `impl {}` is missing validator methods for `{state_enum_name}`.",
            contract.machine_attr_display,
            contract.persisted_type_display,
        ))
        .found(format!("missing validator methods: `{missing_list}`"))
        .expected(format!(
            "one `is_{{state}}` method per `{state_enum_name}` variant"
        ))
        .fix("add one validator per state variant in snake_case, for example `fn is_draft(&self) -> Result<(), _>`.".to_string());
        return Err(compile_error_at(proc_macro2::Span::call_site(), &message));
    }

    Ok(ValidatorPlan { methods, has_async })
}
