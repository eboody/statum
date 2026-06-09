use quote::{format_ident, quote};
use syn::{Generics, Ident, Path, Type};

use super::shared::{
    failed_rebuild_attempt_with_rejection_tokens, machine_builder_path_tokens,
    machine_state_variant_path_tokens, rebuild_attempt_tokens,
};
use crate::validators::contract::{ValidatorMethodContract, ValidatorReturnKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StatePayloadMode {
    Unit,
    StateData,
}

struct PreparedValidatorCheck<'a> {
    method: &'a Ident,
    variant_ident: Ident,
    receiver: &'a proc_macro2::TokenStream,
    field_names: &'a [Ident],
    await_token: proc_macro2::TokenStream,
    machine_builder_path: proc_macro2::TokenStream,
    machine_state_variant_path: proc_macro2::TokenStream,
    payload_mode: StatePayloadMode,
    return_kind: ValidatorReturnKind,
}

impl PreparedValidatorCheck<'_> {
    fn builder_call(&self) -> proc_macro2::TokenStream {
        let machine_builder_path = &self.machine_builder_path;
        let field_names = self.field_names;
        let field_builder_chain = quote! { #(.#field_names(#field_names))* };

        match self.payload_mode {
            StatePayloadMode::StateData => quote! {
                #machine_builder_path::builder()
                    #field_builder_chain
                    .state_data(__statum_state_data)
                    .build()
            },
            StatePayloadMode::Unit => quote! {
                #machine_builder_path::builder()
                    #field_builder_chain
                    .build()
            },
        }
    }
}

pub(crate) struct ValidatorCheckContext<'a> {
    pub(crate) machine_ident: &'a Ident,
    pub(crate) struct_ident: &'a Type,
    pub(crate) candidate_states: &'a [proc_macro2::TokenStream],
    pub(crate) machine_path: &'a Path,
    pub(crate) machine_module_path: &'a Path,
    pub(crate) machine_generics: &'a Generics,
    pub(crate) field_names: &'a [Ident],
    pub(crate) receiver: &'a proc_macro2::TokenStream,
}

pub(crate) fn generate_validator_check(
    context: &ValidatorCheckContext<'_>,
    method: &ValidatorMethodContract,
) -> proc_macro2::TokenStream {
    let prepared = prepare_validator_check(context, method);

    if prepared.payload_mode == StatePayloadMode::StateData {
        let builder_call = prepared.builder_call();
        let receiver = prepared.receiver;
        let validator_fn_ident = prepared.method;
        let field_names = prepared.field_names;
        let await_token = prepared.await_token;
        let machine_state_variant_path = prepared.machine_state_variant_path;
        quote! {
            if let Ok(__statum_state_data) = #receiver.#validator_fn_ident(#(&#field_names),*)#await_token {
                return Ok(#machine_state_variant_path(
                    #builder_call
                ));
            }
        }
    } else {
        let builder_call = prepared.builder_call();
        let receiver = prepared.receiver;
        let validator_fn_ident = prepared.method;
        let field_names = prepared.field_names;
        let await_token = prepared.await_token;
        let machine_state_variant_path = prepared.machine_state_variant_path;
        quote! {
            if #receiver.#validator_fn_ident(#(&#field_names),*)#await_token.is_ok() {
                return Ok(#machine_state_variant_path(
                    #builder_call
                ));
            }
        }
    }
}

pub(crate) fn generate_validator_report_check(
    context: &ValidatorCheckContext<'_>,
    method: &ValidatorMethodContract,
) -> proc_macro2::TokenStream {
    let prepared = prepare_validator_check(context, method);
    let builder_call = prepared.builder_call();
    let field_names = prepared.field_names;
    let receiver = prepared.receiver;
    let validator_fn_ident = prepared.method;
    let await_token = prepared.await_token;
    let variant_ident = prepared.variant_ident;
    let matched_attempt = rebuild_attempt_tokens(validator_fn_ident, &variant_ident, true);
    let failed_attempt = rebuild_attempt_tokens(validator_fn_ident, &variant_ident, false);
    let failed_attempt_with_rejection =
        failed_rebuild_attempt_with_rejection_tokens(validator_fn_ident, &variant_ident);
    let machine_ident = context.machine_ident;
    let struct_ident = context.struct_ident;
    let candidate_states = context.candidate_states;
    let machine_state_variant_path = prepared.machine_state_variant_path;

    if prepared.payload_mode == StatePayloadMode::StateData {
        match prepared.return_kind {
            ValidatorReturnKind::Plain => quote! {
                match #receiver.#validator_fn_ident(#(&#field_names),*)#await_token {
                    Ok(__statum_state_data) => {
                        __statum_attempts.push(#matched_attempt);
                        return statum::RebuildReport::new(
                            stringify!(#machine_ident),
                            statum::__private::RebuildInput {
                                type_name: core::any::type_name::<#struct_ident>(),
                                identifier: core::option::Option::None,
                            },
                            ::std::vec![#(#candidate_states),*],
                            statum::__private::RebuildAmbiguity::NotChecked,
                            __statum_attempts,
                            Ok(#machine_state_variant_path(#builder_call)),
                        );
                    }
                    Err(_) => __statum_attempts.push(#failed_attempt),
                }
            },
            ValidatorReturnKind::Diagnostic => quote! {
                match #receiver.#validator_fn_ident(#(&#field_names),*)#await_token {
                    Ok(__statum_state_data) => {
                        __statum_attempts.push(#matched_attempt);
                        return statum::RebuildReport::new(
                            stringify!(#machine_ident),
                            statum::__private::RebuildInput {
                                type_name: core::any::type_name::<#struct_ident>(),
                                identifier: core::option::Option::None,
                            },
                            ::std::vec![#(#candidate_states),*],
                            statum::__private::RebuildAmbiguity::NotChecked,
                            __statum_attempts,
                            Ok(#machine_state_variant_path(#builder_call)),
                        );
                    }
                    Err(__statum_rejection) => __statum_attempts.push(#failed_attempt_with_rejection),
                }
            },
        }
    } else {
        match prepared.return_kind {
            ValidatorReturnKind::Plain => quote! {
                if #receiver.#validator_fn_ident(#(&#field_names),*)#await_token.is_ok() {
                    __statum_attempts.push(#matched_attempt);
                    return statum::RebuildReport::new(
                        stringify!(#machine_ident),
                        statum::__private::RebuildInput {
                            type_name: core::any::type_name::<#struct_ident>(),
                            identifier: core::option::Option::None,
                        },
                        ::std::vec![#(#candidate_states),*],
                        statum::__private::RebuildAmbiguity::NotChecked,
                        __statum_attempts,
                        Ok(#machine_state_variant_path(#builder_call)),
                    );
                }

                __statum_attempts.push(#failed_attempt);
            },
            ValidatorReturnKind::Diagnostic => quote! {
                match #receiver.#validator_fn_ident(#(&#field_names),*)#await_token {
                    Ok(()) => {
                        __statum_attempts.push(#matched_attempt);
                        return statum::RebuildReport::new(
                            stringify!(#machine_ident),
                            statum::__private::RebuildInput {
                                type_name: core::any::type_name::<#struct_ident>(),
                                identifier: core::option::Option::None,
                            },
                            ::std::vec![#(#candidate_states),*],
                            statum::__private::RebuildAmbiguity::NotChecked,
                            __statum_attempts,
                            Ok(#machine_state_variant_path(#builder_call)),
                        );
                    }
                    Err(__statum_rejection) => {
                        __statum_attempts.push(#failed_attempt_with_rejection);
                    }
                }
            },
        }
    }
}

pub(crate) fn generate_validator_explain_check(
    context: &ValidatorCheckContext<'_>,
    method: &ValidatorMethodContract,
) -> proc_macro2::TokenStream {
    let prepared = prepare_validator_check(context, method);
    let field_names = prepared.field_names;
    let receiver = prepared.receiver;
    let validator_fn_ident = prepared.method;
    let await_token = prepared.await_token;
    let variant_ident = prepared.variant_ident;
    let matched_attempt = rebuild_attempt_tokens(validator_fn_ident, &variant_ident, true);
    let failed_attempt = rebuild_attempt_tokens(validator_fn_ident, &variant_ident, false);
    let failed_attempt_with_rejection =
        failed_rebuild_attempt_with_rejection_tokens(validator_fn_ident, &variant_ident);
    let storage_ident = explain_storage_ident(validator_fn_ident);

    let record_match = quote! {
        __statum_attempts.push(#matched_attempt);
        __statum_matched_states.push(stringify!(#variant_ident));
    };

    if prepared.payload_mode == StatePayloadMode::StateData {
        match prepared.return_kind {
            ValidatorReturnKind::Plain => quote! {
                match #receiver.#validator_fn_ident(#(&#field_names),*)#await_token {
                    Ok(__statum_state_data) => {
                        #storage_ident = core::option::Option::Some(__statum_state_data);
                        #record_match
                    }
                    Err(_) => __statum_attempts.push(#failed_attempt),
                }
            },
            ValidatorReturnKind::Diagnostic => quote! {
                match #receiver.#validator_fn_ident(#(&#field_names),*)#await_token {
                    Ok(__statum_state_data) => {
                        #storage_ident = core::option::Option::Some(__statum_state_data);
                        #record_match
                    }
                    Err(__statum_rejection) => {
                        __statum_attempts.push(#failed_attempt_with_rejection);
                    }
                }
            },
        }
    } else {
        match prepared.return_kind {
            ValidatorReturnKind::Plain => quote! {
                if #receiver.#validator_fn_ident(#(&#field_names),*)#await_token.is_ok() {
                    #record_match
                } else {
                    __statum_attempts.push(#failed_attempt);
                }
            },
            ValidatorReturnKind::Diagnostic => quote! {
                match #receiver.#validator_fn_ident(#(&#field_names),*)#await_token {
                    Ok(()) => { #record_match }
                    Err(__statum_rejection) => {
                        __statum_attempts.push(#failed_attempt_with_rejection);
                    }
                }
            },
        }
    }
}

pub(crate) fn generate_validator_explain_storage(
    method: &ValidatorMethodContract,
) -> proc_macro2::TokenStream {
    if !method.has_state_data {
        return quote! {};
    }

    let storage_ident = explain_storage_ident(&method.validator_fn);
    let ok_type = &method.ok_type;
    quote! {
        let mut #storage_ident: core::option::Option<#ok_type> = core::option::Option::None;
    }
}

pub(crate) fn generate_validator_explain_finalizer(
    context: &ValidatorCheckContext<'_>,
    method: &ValidatorMethodContract,
) -> proc_macro2::TokenStream {
    let prepared = prepare_validator_check(context, method);
    let builder_call = prepared.builder_call();
    let variant_ident = prepared.variant_ident;
    let machine_state_variant_path = prepared.machine_state_variant_path;

    if prepared.payload_mode == StatePayloadMode::StateData {
        let storage_ident = explain_storage_ident(prepared.method);
        let message = format!(
            "statum internal error: `{}` accepted during explain without stored state data",
            prepared.method
        );
        quote! {
            [stringify!(#variant_ident)] => {
                let __statum_state_data = #storage_ident.expect(#message);
                Ok(#machine_state_variant_path(#builder_call))
            }
        }
    } else {
        quote! {
            [stringify!(#variant_ident)] => Ok(#machine_state_variant_path(#builder_call))
        }
    }
}

fn explain_storage_ident(validator_fn_ident: &Ident) -> Ident {
    format_ident!("__statum_explain_{}_state_data", validator_fn_ident)
}

fn prepare_validator_check<'a>(
    context: &'a ValidatorCheckContext<'a>,
    method: &'a ValidatorMethodContract,
) -> PreparedValidatorCheck<'a> {
    let machine_path = context.machine_path;
    let machine_module_path = context.machine_module_path;
    let machine_generics = context.machine_generics;
    let field_names = context.field_names;
    let receiver = context.receiver;
    let variant_ident = format_ident!("{}", method.variant_name);
    let validator_fn_ident = &method.validator_fn;
    let await_token = if method.is_async {
        quote! { .await }
    } else {
        quote! {}
    };
    let machine_builder_path =
        machine_builder_path_tokens(machine_path, machine_generics, &variant_ident);
    let machine_state_variant_path =
        machine_state_variant_path_tokens(machine_module_path, machine_generics, &variant_ident);
    let payload_mode = if method.has_state_data {
        StatePayloadMode::StateData
    } else {
        StatePayloadMode::Unit
    };

    PreparedValidatorCheck {
        method: validator_fn_ident,
        variant_ident,
        receiver,
        field_names,
        await_token,
        machine_builder_path,
        machine_state_variant_path,
        payload_mode,
        return_kind: method.return_kind,
    }
}
