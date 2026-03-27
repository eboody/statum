use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{Ident, ItemImpl, Type, parse_macro_input};

mod emission;
mod resolution;
mod signatures;

use emission::{
    emit_validator_methods_impl as emit_validator_methods_impl_inner,
    generate_validator_build_variant_macro, generate_validator_report_variant_macro,
    validator_support_macro_ident,
};
use resolution::resolve_machine_metadata;
use signatures::{
    AnalyzedValidatorReturn, ValidatorDiagnosticContext, ValidatorReturnKind,
    analyze_validator_return_type, validate_validator_signature, validator_state_name_from_ident,
};
use crate::parse_doc_attrs;

pub(super) struct ValidatorMethodSpec {
    pub(super) validator_ident: Ident,
    pub(super) variant_name: String,
    pub(super) actual_ok_type: Type,
    pub(super) return_kind: ValidatorReturnKind,
    pub(super) is_async: bool,
}

struct CollectValidatorContext<'a> {
    machine_name: &'a str,
    persisted_type_display: &'a str,
    state_enum_name: &'a str,
    machine_fields: Option<&'a [Ident]>,
}

pub fn parse_validators(
    attr: TokenStream,
    item: TokenStream,
    module_path: &str,
    line_number: usize,
) -> TokenStream {
    let machine_ident = parse_macro_input!(attr as Ident);
    let item_impl = parse_macro_input!(item as ItemImpl);
    let struct_ident = &item_impl.self_ty;
    let persisted_type_display = struct_ident.to_token_stream().to_string();
    let machine_name = machine_ident.to_string();
    let diagnostic_machine = match resolve_machine_metadata(module_path, &machine_ident) {
        Ok(machine) => machine,
        Err(err) => return err.into(),
    };
    let state_enum_name = diagnostic_machine
        .state_generic_name
        .as_deref()
        .unwrap_or("State")
        .to_string();
    let diagnostic_machine_fields = diagnostic_machine
        .parse()
        .ok()
        .map(|parsed| {
            parsed
                .field_idents_and_types()
                .into_iter()
                .map(|(ident, _)| ident)
                .collect::<Vec<_>>()
        });

    let collect_context = CollectValidatorContext {
        machine_name: &machine_name,
        persisted_type_display: &persisted_type_display,
        state_enum_name: &state_enum_name,
        machine_fields: diagnostic_machine_fields.as_deref(),
    };
    let validator_methods = match collect_validator_methods(&item_impl, &collect_context) {
        Ok(result) => result,
        Err(err) => return err.into(),
    };

    let machine_validator_contract_macro_ident =
        format_ident!("__statum_visit_{}_validators", crate::to_snake_case(&machine_name));
    let validator_support_macro_ident = validator_support_macro_ident(&machine_name);
    let validator_method_items = item_impl
        .items
        .iter()
        .filter_map(|item| match item {
            syn::ImplItem::Fn(func)
                if validator_state_name_from_ident(&func.sig.ident).is_some() =>
            {
                Some(quote! { #func })
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let passthrough_items = item_impl
        .items
        .iter()
        .filter(|item| match item {
            syn::ImplItem::Fn(func) => validator_state_name_from_ident(&func.sig.ident).is_none(),
            _ => true,
        })
        .collect::<Vec<_>>();
    let build_variant_macro =
        generate_validator_build_variant_macro(&machine_name, &validator_methods);
    let report_variant_macro =
        generate_validator_report_variant_macro(&machine_name, &validator_methods);
    let validator_contract_checks = if item_impl.items.is_empty() {
        generate_empty_validator_methods_error(
            &machine_validator_contract_macro_ident,
            &machine_ident,
            &persisted_type_display,
        )
    } else {
        generate_authoritative_validator_coverage(
            &machine_validator_contract_macro_ident,
            &machine_ident,
            &persisted_type_display,
            &validator_methods,
        )
    };
    let build_variant_macro_ident = format_ident!(
        "__statum_emit_{}_validator_build_variant",
        crate::to_snake_case(&machine_name)
    );
    let report_variant_macro_ident = format_ident!(
        "__statum_emit_{}_validator_report_variant",
        crate::to_snake_case(&machine_name)
    );
    let machine_module_path = syn::LitStr::new(
        diagnostic_machine.module_path.as_ref(),
        proc_macro2::Span::call_site(),
    );
    let machine_rust_type_path = syn::LitStr::new(
        &format!("{}::{}", diagnostic_machine.module_path, machine_name),
        proc_macro2::Span::call_site(),
    );
    let linked_validator_registration_ident = linked_validator_registration_ident(
        &machine_name,
        module_path,
        &persisted_type_display,
        line_number,
    );
    let linked_validator_targets_ident = linked_validator_targets_ident(
        &machine_name,
        module_path,
        &persisted_type_display,
        line_number,
    );
    let source_module_path = syn::LitStr::new(module_path, proc_macro2::Span::call_site());
    let source_type_display =
        syn::LitStr::new(&persisted_type_display, proc_macro2::Span::call_site());
    let docs = match parse_doc_attrs(&item_impl.attrs) {
        Ok(docs) => docs,
        Err(err) => return err.to_compile_error().into(),
    };
    let linked_docs = optional_lit_str_tokens(docs.as_deref());
    let validator_target_states = validator_methods
        .iter()
        .map(|method| syn::LitStr::new(&method.variant_name, proc_macro2::Span::call_site()))
        .collect::<Vec<_>>();
    let linked_validator_registration = quote! {
        #[doc(hidden)]
        static #linked_validator_targets_ident: &[&str] = &[#(#validator_target_states),*];

        #[doc(hidden)]
        #[statum::__private::linkme::distributed_slice(statum::__private::__STATUM_LINKED_VALIDATOR_ENTRIES)]
        #[linkme(crate = statum::__private::linkme)]
        static #linked_validator_registration_ident: statum::__private::LinkedValidatorEntryDescriptor =
            statum::__private::LinkedValidatorEntryDescriptor {
                machine: statum::MachineDescriptor {
                    module_path: #machine_module_path,
                    rust_type_path: #machine_rust_type_path,
                },
                source_module_path: #source_module_path,
                source_type_display: #source_type_display,
                docs: #linked_docs,
                target_states: #linked_validator_targets_ident,
            };
    };
    let async_mode = if validator_methods.iter().any(|method| method.is_async) {
        quote! { true }
    } else {
        quote! { false }
    };
    let validator_count = validator_methods.len();
    let passthrough_impl = if passthrough_items.is_empty() {
        quote! {}
    } else {
        quote! {
            impl #struct_ident {
                #(#passthrough_items)*
            }
        }
    };

    quote! {
        #validator_contract_checks
        #passthrough_impl
        #build_variant_macro
        #report_variant_macro
        #linked_validator_registration
        #validator_support_macro_ident! {
            persisted = #struct_ident,
            build_variant = #build_variant_macro_ident,
            report_variant = #report_variant_macro_ident,
            validator_count = #validator_count,
            async = #async_mode,
            validator_methods = [
                #(#validator_method_items),*
            ],
        }
    }
    .into()
}

pub(crate) fn emit_validator_methods_impl(input: TokenStream) -> TokenStream {
    emit_validator_methods_impl_inner(input)
}

fn collect_validator_methods(
    item_impl: &ItemImpl,
    context: &CollectValidatorContext<'_>,
) -> Result<Vec<ValidatorMethodSpec>, proc_macro2::TokenStream> {
    let mut methods = Vec::new();
    for item in &item_impl.items {
        let syn::ImplItem::Fn(func) = item else {
            continue;
        };

        let Some(state_name) = validator_state_name_from_ident(&func.sig.ident) else {
            continue;
        };
        if let Some(attr_name) = cfg_like_attr_name(&func.attrs) {
            let message = format!(
                "Error: `#[validators({})]` on `impl {}` method `{}` uses `#[{}]`, but Statum does not support conditionally compiled validator methods.\nFix: move the cfg gate to the whole `#[validators({})]` impl or split cfg-specific rebuild surfaces into separate impls.",
                context.machine_name,
                context.persisted_type_display,
                func.sig.ident,
                attr_name,
                context.machine_name,
            );
            return Err(syn::Error::new_spanned(func, message).to_compile_error());
        }
        let diagnostic_context = ValidatorDiagnosticContext {
            persisted_type_display: context.persisted_type_display,
            machine_name: context.machine_name,
            state_enum_name: context.state_enum_name,
            variant_name: &state_name_to_variant_name(&state_name),
            machine_fields: context.machine_fields,
        };
        validate_validator_signature(func, &diagnostic_context)?;
        let AnalyzedValidatorReturn {
            ok_type,
            return_kind,
        } = analyze_validator_return_type(func, &diagnostic_context)?;
        let variant_name = state_name_to_variant_name(&state_name);
        methods.push(ValidatorMethodSpec {
            validator_ident: func.sig.ident.clone(),
            variant_name,
            actual_ok_type: ok_type,
            return_kind,
            is_async: func.sig.asyncness.is_some(),
        });
    }

    Ok(methods)
}

fn state_name_to_variant_name(state_name: &str) -> String {
    let mut result = String::new();
    for segment in state_name.split('_').filter(|segment| !segment.is_empty()) {
        let mut chars = segment.chars();
        if let Some(first) = chars.next() {
            result.extend(first.to_uppercase());
            result.push_str(chars.as_str());
        }
    }
    result
}

fn cfg_like_attr_name(attrs: &[syn::Attribute]) -> Option<&'static str> {
    attrs.iter().find_map(|attr| {
        if attr.path().is_ident("cfg") {
            Some("cfg")
        } else if attr.path().is_ident("cfg_attr") {
            Some("cfg_attr")
        } else {
            None
        }
    })
}

fn linked_validator_registration_ident(
    machine_name: &str,
    module_path: &str,
    persisted_type_display: &str,
    line_number: usize,
) -> Ident {
    let key = format!(
        "{machine_name}::validator-entry::{module_path}::{persisted_type_display}::{line_number}"
    );
    format_ident!("__STATUM_LINKED_VALIDATOR_ENTRY_{:016X}", stable_hash(&key))
}

fn linked_validator_targets_ident(
    machine_name: &str,
    module_path: &str,
    persisted_type_display: &str,
    line_number: usize,
) -> Ident {
    let key = format!(
        "{machine_name}::validator-targets::{module_path}::{persisted_type_display}::{line_number}"
    );
    format_ident!(
        "__STATUM_LINKED_VALIDATOR_TARGETS_{:016X}",
        stable_hash(&key)
    )
}

fn stable_hash(input: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn optional_lit_str_tokens(value: Option<&str>) -> proc_macro2::TokenStream {
    match value {
        Some(value) => {
            let value = syn::LitStr::new(value, proc_macro2::Span::call_site());
            quote! { Some(#value) }
        }
        None => quote! { None },
    }
}

fn generate_empty_validator_methods_error(
    machine_validator_contract_macro_ident: &Ident,
    machine_ident: &Ident,
    persisted_type_display: &str,
) -> proc_macro2::TokenStream {
    let emit_error_macro_ident = format_ident!(
        "__statum_emit_{}_validator_no_methods",
        crate::to_snake_case(&machine_ident.to_string())
    );

    quote! {
        #[doc(hidden)]
        macro_rules! #emit_error_macro_ident {
            (
                machine = $machine:ident,
                state_family = $state_family:ident,
                state_trait = $state_trait:ident,
                machine_module = $machine_module:ident,
                machine_vis = $machine_vis:vis,
                extra_generics = $extra_generics:tt,
                fields = $fields:tt,
                variants = [
                    {
                        marker = $first_marker:ident,
                        validator = $first_validator:ident,
                        $($first_variant_rest:tt)*
                    }
                    $(
                        ,
                        {
                            marker = $marker:ident,
                            validator = $validator:ident,
                            $($variant_rest:tt)*
                        }
                    )* $(,)?
                ],
                $($rest:tt)*
            ) => {
                compile_error!(concat!(
                    "Error: `#[validators(",
                    stringify!($machine),
                    ")]` on `impl ",
                    #persisted_type_display,
                    "` must define at least one validator method.\n",
                    "Expected one method per `",
                    stringify!($state_family),
                    "` variant: ",
                    stringify!($first_validator),
                    $(", ", stringify!($validator),)*
                    "."
                ));
            };
        }

        #machine_validator_contract_macro_ident!(#emit_error_macro_ident);
    }
}

fn generate_authoritative_validator_coverage(
    machine_validator_contract_macro_ident: &Ident,
    machine_ident: &Ident,
    persisted_type_display: &str,
    validator_methods: &[ValidatorMethodSpec],
) -> proc_macro2::TokenStream {
    let known_validator_macro_ident = format_ident!(
        "__statum_assert_{}_known_validator",
        crate::to_snake_case(&machine_ident.to_string())
    );
    let emit_known_validator_macro_ident = format_ident!(
        "__statum_emit_{}_known_validators",
        crate::to_snake_case(&machine_ident.to_string())
    );
    let present_validator_macro_ident = format_ident!(
        "__statum_assert_{}_validator_present",
        crate::to_snake_case(&machine_ident.to_string())
    );
    let emit_present_validator_macro_ident = format_ident!(
        "__statum_emit_{}_validator_presence",
        crate::to_snake_case(&machine_ident.to_string())
    );
    let validator_idents = validator_methods
        .iter()
        .map(|method| method.validator_ident.clone())
        .collect::<Vec<_>>();

    quote! {
        #[doc(hidden)]
        macro_rules! #emit_known_validator_macro_ident {
            (
                machine = $machine:ident,
                state_family = $state_family:ident,
                state_trait = $state_trait:ident,
                machine_module = $machine_module:ident,
                machine_vis = $machine_vis:vis,
                extra_generics = $extra_generics:tt,
                fields = $fields:tt,
                variants = [
                    {
                        marker = $first_marker:ident,
                        validator = $first_validator:ident,
                        $($first_variant_rest:tt)*
                    }
                    $(
                        ,
                        {
                            marker = $marker:ident,
                            validator = $validator:ident,
                            $($variant_rest:tt)*
                        }
                    )* $(,)?
                ],
                $($rest:tt)*
            ) => {
                #[doc(hidden)]
                macro_rules! #known_validator_macro_ident {
                    ($first_validator) => {};
                    $(
                        ($validator) => {};
                    )*
                    ($unknown:ident) => {
                        compile_error!(concat!(
                            "Error: `#[validators(",
                            stringify!($machine),
                            ")]` on `impl ",
                            #persisted_type_display,
                            "` defines methods that do not match any variant in `",
                            stringify!($state_family),
                            "`: ",
                            stringify!($unknown),
                            ".\n",
                            "Valid validator methods for `",
                            stringify!($machine),
                            "` are: ",
                            stringify!($first_validator),
                            $(", ", stringify!($validator),)*
                            "."
                        ));
                    };
                }
            };
        }

        #[doc(hidden)]
        macro_rules! #present_validator_macro_ident {
            #(
                (
                    machine = $machine:ident,
                    state_family = $state_family:ident,
                    validator = #validator_idents,
                    state = $variant:ident,
                ) => {};
            )*
            (
                machine = $machine:ident,
                state_family = $state_family:ident,
                validator = $missing:ident,
                state = $variant:ident,
            ) => {
                compile_error!(concat!(
                    "Error: `#[validators(",
                    stringify!($machine),
                    ")]` on `impl ",
                    #persisted_type_display,
                    "` is missing validator method `",
                    stringify!($missing),
                    "` for `",
                    stringify!($state_family),
                    "::",
                    stringify!($variant),
                    "`.\n",
                    "Fix: add one validator per state variant (snake_case), e.g. `fn is_draft(&self) -> Result<()>`."
                ));
            };
        }

        #[doc(hidden)]
        macro_rules! #emit_present_validator_macro_ident {
            (
                machine = $machine:ident,
                state_family = $state_family:ident,
                state_trait = $state_trait:ident,
                machine_module = $machine_module:ident,
                machine_vis = $machine_vis:vis,
                extra_generics = $extra_generics:tt,
                fields = $fields:tt,
                variants = [
                    $(
                        {
                            marker = $variant:ident,
                            validator = $validator:ident,
                            $($variant_rest:tt)*
                        }
                    ),* $(,)?
                ],
                $($rest:tt)*
            ) => {
                $(
                    #present_validator_macro_ident!(
                        machine = $machine,
                        state_family = $state_family,
                        validator = $validator,
                        state = $variant,
                    );
                )*
            };
        }

        #machine_validator_contract_macro_ident!(#emit_known_validator_macro_ident);
        #(
            #known_validator_macro_ident!(#validator_idents);
        )*
        #machine_validator_contract_macro_ident!(#emit_present_validator_macro_ident);
    }
}
