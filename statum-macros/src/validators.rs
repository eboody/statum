//! `#[validators]` subsystem: resolve target machines, validate signatures, and emit rebuild helpers.

use quote::{ToTokens, quote};
use std::marker::PhantomData;
use syn::{ImplItem, ItemImpl, Path};

use crate::contracts::ValidatorContract;
use crate::diagnostics::{DiagnosticMessage, compile_error_at};
use crate::machine::extra_generics;

mod contract;
mod emission;
mod plan;
mod resolution;
mod signatures;
mod type_equivalence;

use contract::{
    build_validator_contract, qualify_machine_field_types,
};
use emission::{
    ValidatorBuilderSurfaceContext, ValidatorCheckContext, generate_validator_check,
    generate_validator_report_check, inject_machine_fields, validator_builder_surface,
};
use plan::collect_validator_plan;
use resolution::{
    resolve_machine_metadata, resolve_state_enum_info, resolve_validator_machine_attr,
};

use self::contract::ValidatorPlan;

pub fn parse_validators(
    attr: proc_macro::TokenStream,
    item_impl: ItemImpl,
    module_path: &str,
) -> proc_macro::TokenStream {
    ValidatorsExpansionBuilder::<ParsedValidatorsPhase>::parse(attr, item_impl, module_path)
        .and_then(ValidatorsExpansionBuilder::<ParsedValidatorsPhase>::resolve)
        .and_then(ValidatorsExpansionBuilder::<ResolvedValidatorsPhase>::plan)
        .map(ValidatorsExpansionBuilder::<PlannedValidatorsPhase>::emit)
        .unwrap_or_else(Into::into)
}

struct ParsedValidatorsPhase;
struct ResolvedValidatorsPhase;
struct PlannedValidatorsPhase;

struct ValidatorsExpansionBuilder<State> {
    item_impl: ItemImpl,
    machine_path: Path,
    persisted_type_display: String,
    module_path: String,
    modified_methods: Vec<ImplItem>,
    contract: Option<ValidatorContract>,
    validator_plan: Option<ValidatorPlan>,
    _state: PhantomData<State>,
}

impl ValidatorsExpansionBuilder<ParsedValidatorsPhase> {
    fn parse(
        attr: proc_macro::TokenStream,
        item_impl: ItemImpl,
        module_path: &str,
    ) -> Result<Self, proc_macro2::TokenStream> {
        let machine_path = syn::parse::<Path>(attr).map_err(|err| err.to_compile_error())?;
        let persisted_type_display = item_impl.self_ty.to_token_stream().to_string();
        Ok(Self {
            item_impl,
            machine_path,
            persisted_type_display,
            module_path: module_path.to_string(),
            modified_methods: Vec::new(),
            contract: None,
            validator_plan: None,
            _state: PhantomData,
        })
    }

    fn resolve(self) -> Result<ValidatorsExpansionBuilder<ResolvedValidatorsPhase>, proc_macro2::TokenStream> {
        let machine_attr = resolve_validator_machine_attr(&self.module_path, &self.machine_path)?;
        let machine_metadata = resolve_machine_metadata(&self.module_path, &machine_attr)?;
        let parsed_machine = machine_metadata.parse()?;
        let parsed_fields = qualify_machine_field_types(
            &parsed_machine.field_idents_and_types(),
            &machine_attr.machine_path,
        );
        let validator_machine_generics = extra_generics(&parsed_machine.generics);
        let modified_methods = inject_machine_fields(
            &self.item_impl.items,
            &parsed_fields,
            &validator_machine_generics,
        )?;
        let state_enum_info = resolve_state_enum_info(&machine_metadata)?;
        let contract = build_validator_contract(
            &machine_attr,
            parsed_machine,
            &parsed_fields,
            state_enum_info,
            &self.persisted_type_display,
        );

        Ok(ValidatorsExpansionBuilder {
            item_impl: self.item_impl,
            machine_path: self.machine_path,
            persisted_type_display: self.persisted_type_display,
            module_path: self.module_path,
            modified_methods,
            contract: Some(contract),
            validator_plan: None,
            _state: PhantomData,
        })
    }
}

impl ValidatorsExpansionBuilder<ResolvedValidatorsPhase> {
    fn plan(self) -> Result<ValidatorsExpansionBuilder<PlannedValidatorsPhase>, proc_macro2::TokenStream> {
        let contract = self.contract.as_ref().ok_or_else(|| {
            compile_error_at(
                proc_macro2::Span::call_site(),
                &DiagnosticMessage::new(
                    "internal Statum error: `#[validators]` pipeline reached planning without a resolved validator contract.",
                ),
            )
        })?;
        let has_any_methods = self
            .item_impl
            .items
            .iter()
            .any(|item| matches!(item, syn::ImplItem::Fn(_)));
        if !has_any_methods {
            let expected_methods = contract
                .state_enum
                .variants
                .iter()
                .map(|variant| format!("is_{}", crate::to_snake_case(&variant.name)))
                .collect::<Vec<_>>()
                .join(", ");
            let state_enum_name = contract.state_enum.name.clone();
            let message = DiagnosticMessage::new(format!(
                "`#[validators({})]` on `impl {}` must define at least one validator method.",
                contract.machine_attr_display, contract.persisted_type_display,
            ))
            .expected(format!(
                "one method per `{state_enum_name}` variant: `{expected_methods}`"
            ))
            .fix("add validator methods like `fn is_draft(&self) -> Result<(), _>`.".to_string());
            return Err(compile_error_at(proc_macro2::Span::call_site(), &message));
        }

        let validator_plan = collect_validator_plan(&self.item_impl, contract)?;
        Ok(ValidatorsExpansionBuilder {
            item_impl: self.item_impl,
            machine_path: self.machine_path,
            persisted_type_display: self.persisted_type_display,
            module_path: self.module_path,
            modified_methods: self.modified_methods,
            contract: self.contract,
            validator_plan: Some(validator_plan),
            _state: PhantomData,
        })
    }
}

impl ValidatorsExpansionBuilder<PlannedValidatorsPhase> {
    fn emit(self) -> proc_macro::TokenStream {
        let struct_ident = &self.item_impl.self_ty;
        let machine_path = &self.machine_path;
        let modified_methods = &self.modified_methods;
        let contract = match self.contract {
            Some(contract) => contract,
            None => {
                return compile_error_at(
                    proc_macro2::Span::call_site(),
                    &DiagnosticMessage::new(
                        "internal Statum error: planned `#[validators]` pipeline reached emission without a resolved validator contract.",
                    ),
                )
                .into();
            }
        };
        let validator_plan = match self.validator_plan {
            Some(validator_plan) => validator_plan,
            None => {
                return compile_error_at(
                    proc_macro2::Span::call_site(),
                    &DiagnosticMessage::new(
                        "internal Statum error: planned `#[validators]` pipeline reached emission without a validator plan.",
                    ),
                )
                .into();
            }
        };
        let ValidatorContract {
            resolved_machine,
            state_enum,
            ..
        } = contract;

        let receiver = quote! { __statum_persisted };
        let emission_context = ValidatorCheckContext {
            machine_path: &resolved_machine.machine_path,
            machine_module_path: &resolved_machine.machine_module_path,
            machine_generics: resolved_machine.machine_generics(),
            field_names: &resolved_machine.field_names,
            receiver: &receiver,
        };

        let validator_checks = validator_plan
            .methods
            .iter()
            .map(|method| generate_validator_check(&emission_context, method))
            .collect::<Vec<_>>();
        let validator_report_checks = validator_plan
            .methods
            .iter()
            .map(|method| generate_validator_report_check(&emission_context, method))
            .collect::<Vec<_>>();

        let machine_vis = resolved_machine.parsed_machine.vis.clone();
        let async_token = if validator_plan.has_async {
            quote! { async }
        } else {
            quote! {}
        };

        validator_builder_surface(ValidatorBuilderSurfaceContext {
            machine_ident: &resolved_machine.machine_ident,
            machine_path,
            machine_module_path: &resolved_machine.machine_module_path,
            machine_generics: resolved_machine.machine_generics(),
            struct_ident,
            state_enum_name: &state_enum.name,
            machine_state_ty: &resolved_machine.machine_state_ty,
            field_names: &resolved_machine.field_names,
            field_types: &resolved_machine.field_types,
            validator_checks: &validator_checks,
            validator_report_checks: &validator_report_checks,
            modified_methods,
            async_token: &async_token,
            machine_vis: &machine_vis,
        })
        .into()
    }
}
