use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{Generics, Ident, ItemImpl, Type, parse_macro_input};

use crate::machine::{
    builder_generics, extra_generics, extra_type_arguments_tokens, generic_argument_tokens,
};

mod emission;
mod resolution;
mod signatures;

use emission::{
    BatchBuilderContext, ValidatorCheckContext, batch_builder_implementation,
    generate_validator_check_template, generate_validator_report_check_template,
    inject_machine_fields,
};
use resolution::resolve_machine_metadata;
use signatures::{
    AnalyzedValidatorReturn, ValidatorDiagnosticContext, ValidatorReturnKind,
    analyze_validator_return_type, validate_validator_signature, validator_state_name_from_ident,
};

struct ValidatorMethodSpec {
    validator_ident: Ident,
    actual_ok_type: Type,
    return_kind: ValidatorReturnKind,
    is_async: bool,
}

struct CollectValidatorContext<'a> {
    machine_ident: &'a Ident,
    machine_module_ident: &'a Ident,
    machine_generics: &'a Generics,
    field_names: &'a [Ident],
    persisted_type_display: &'a str,
    machine_name: &'a str,
    state_enum_name: &'a str,
}

struct IntoMachineBuilderContext<'a> {
    builder_ident: &'a Ident,
    struct_ident: &'a Type,
    machine_generics: &'a Generics,
    machine_state_ty: &'a proc_macro2::TokenStream,
    field_names: &'a [Ident],
    field_types: &'a [Type],
    validator_checks: &'a [proc_macro2::TokenStream],
    validator_report_checks: &'a [proc_macro2::TokenStream],
    async_token: &'a proc_macro2::TokenStream,
    machine_vis: &'a syn::Visibility,
}

pub fn parse_validators(attr: TokenStream, item: TokenStream, module_path: &str) -> TokenStream {
    let machine_ident = parse_macro_input!(attr as Ident);
    let item_impl = parse_macro_input!(item as ItemImpl);
    let struct_ident = &item_impl.self_ty;
    let persisted_type_display = struct_ident.to_token_stream().to_string();

    let machine_metadata = match resolve_machine_metadata(module_path, &machine_ident) {
        Ok(metadata) => metadata,
        Err(err) => return err.into(),
    };

    let parsed_machine = match machine_metadata.parse() {
        Ok(parsed) => parsed,
        Err(err) => return err.into(),
    };
    let parsed_fields = parsed_machine.field_idents_and_types();

    let validator_machine_generics = extra_generics(&parsed_machine.generics);
    let modified_methods = match inject_machine_fields(
        &item_impl.items,
        &parsed_fields,
        &validator_machine_generics,
    ) {
        Ok(methods) => methods,
        Err(err) => return err.into(),
    };

    let field_names = parsed_fields
        .iter()
        .map(|(ident, _)| ident.clone())
        .collect::<Vec<_>>();
    let field_types = parsed_fields
        .iter()
        .map(|(_, ty)| ty.clone())
        .collect::<Vec<_>>();
    let machine_module_ident = format_ident!("{}", crate::to_snake_case(&machine_ident.to_string()));
    let machine_extra_ty_args = extra_type_arguments_tokens(&parsed_machine.generics);
    let machine_state_ty = quote! { #machine_module_ident::SomeState #machine_extra_ty_args };
    let machine_name = machine_ident.to_string();
    let state_enum_name = machine_metadata
        .state_generic_name
        .clone()
        .unwrap_or_else(|| "State".to_string());
    let machine_validator_contract_macro_ident =
        format_ident!("__statum_visit_{}_validators", crate::to_snake_case(&machine_name));

    let collect_context = CollectValidatorContext {
        machine_ident: &machine_ident,
        machine_module_ident: &machine_module_ident,
        machine_generics: &parsed_machine.generics,
        field_names: &field_names,
        persisted_type_display: &persisted_type_display,
        machine_name: &machine_name,
        state_enum_name: &state_enum_name,
    };

    let validator_methods = match collect_validator_methods(&item_impl, &collect_context) {
        Ok(result) => result,
        Err(err) => return err.into(),
    };

    let has_async = validator_methods.iter().any(|method| method.is_async);
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
    let validator_checks = vec![generate_authoritative_validator_checks(
        &machine_validator_contract_macro_ident,
        &validator_methods,
        &collect_context,
    )];
    let validator_report_checks = vec![generate_authoritative_validator_report_checks(
        &machine_validator_contract_macro_ident,
        &validator_methods,
        &collect_context,
    )];

    let machine_vis = parsed_machine.vis.clone();

    let async_token = if has_async {
        quote! { async }
    } else {
        quote! {}
    };

    let batch_builder_impl = batch_builder_implementation(BatchBuilderContext {
        machine_ident: &machine_ident,
        machine_module_ident: &machine_module_ident,
        machine_generics: &parsed_machine.generics,
        struct_ident,
        machine_state_ty: &machine_state_ty,
        field_names: &field_names,
        field_types: &field_types,
        async_token: async_token.clone(),
        machine_vis: machine_vis.clone(),
    });

    let into_machine_builder_ident = format_ident!("__Statum{}IntoMachine", machine_ident);
    let into_machine_builder_impl = generate_into_machine_builder(IntoMachineBuilderContext {
        builder_ident: &into_machine_builder_ident,
        struct_ident,
        machine_generics: &parsed_machine.generics,
        machine_state_ty: &machine_state_ty,
        field_names: &field_names,
        field_types: &field_types,
        validator_checks: &validator_checks,
        validator_report_checks: &validator_report_checks,
        async_token: &async_token,
        machine_vis: &machine_vis,
    });
    let into_machine_extra_generics = extra_generics(&parsed_machine.generics);
    let (into_machine_method_generics, _, into_machine_method_where_clause) =
        into_machine_extra_generics.split_for_impl();
    let into_machine_slot_defaults = (0..field_names.len())
        .map(|_| quote! { false })
        .collect::<Vec<_>>();
    let into_machine_builder_ty_generics = generic_argument_tokens(
        into_machine_extra_generics.params.iter(),
        Some(quote! { '_ }),
        &into_machine_slot_defaults,
    );

    let machine_builder_impl = quote! {
        #[allow(unused_imports)]
        use #machine_module_ident::IntoMachinesExt as _;

        impl #struct_ident {
            #machine_vis fn into_machine #into_machine_method_generics (&self) -> #into_machine_builder_ident #into_machine_builder_ty_generics #into_machine_method_where_clause {
                #into_machine_builder_ident {
                    __statum_item: self,
                    #(
                        #field_names: core::option::Option::None
                    ),*
                }
            }

            #(#modified_methods)*
        }

        #into_machine_builder_impl
        #batch_builder_impl
    };

    let expanded = quote! {
        #validator_contract_checks
        #machine_builder_impl
    };

    expanded.into()
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
        let diagnostic_context = ValidatorDiagnosticContext {
            persisted_type_display: context.persisted_type_display,
            machine_name: context.machine_name,
            state_enum_name: context.state_enum_name,
            variant_name: &state_name_to_variant_name(&state_name),
            machine_fields: context.field_names,
        };
        validate_validator_signature(func, &diagnostic_context)?;
        let AnalyzedValidatorReturn {
            ok_type,
            return_kind,
        } = analyze_validator_return_type(func, &diagnostic_context)?;
        methods.push(ValidatorMethodSpec {
            validator_ident: func.sig.ident.clone(),
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

fn generate_empty_validator_methods_error(
    machine_validator_contract_macro_ident: &Ident,
    machine_ident: &Ident,
    persisted_type_display: &str,
) -> proc_macro2::TokenStream {
    let emit_error_macro_ident =
        format_ident!("__statum_emit_{}_validator_no_methods", crate::to_snake_case(&machine_ident.to_string()));

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
    let known_validator_macro_ident =
        format_ident!("__statum_assert_{}_known_validator", crate::to_snake_case(&machine_ident.to_string()));
    let emit_known_validator_macro_ident =
        format_ident!("__statum_emit_{}_known_validators", crate::to_snake_case(&machine_ident.to_string()));
    let present_validator_macro_ident =
        format_ident!("__statum_assert_{}_validator_present", crate::to_snake_case(&machine_ident.to_string()));
    let emit_present_validator_macro_ident =
        format_ident!("__statum_emit_{}_validator_presence", crate::to_snake_case(&machine_ident.to_string()));
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

fn generate_authoritative_validator_checks(
    machine_validator_contract_macro_ident: &Ident,
    validator_methods: &[ValidatorMethodSpec],
    context: &CollectValidatorContext<'_>,
) -> proc_macro2::TokenStream {
    generate_authoritative_validator_body(
        machine_validator_contract_macro_ident,
        validator_methods,
        context,
        ValidatorBodyKind::Build,
    )
}

fn generate_authoritative_validator_report_checks(
    machine_validator_contract_macro_ident: &Ident,
    validator_methods: &[ValidatorMethodSpec],
    context: &CollectValidatorContext<'_>,
) -> proc_macro2::TokenStream {
    generate_authoritative_validator_body(
        machine_validator_contract_macro_ident,
        validator_methods,
        context,
        ValidatorBodyKind::Report,
    )
}

#[derive(Clone, Copy)]
enum ValidatorBodyKind {
    Build,
    Report,
}

fn generate_authoritative_validator_body(
    machine_validator_contract_macro_ident: &Ident,
    validator_methods: &[ValidatorMethodSpec],
    context: &CollectValidatorContext<'_>,
    body_kind: ValidatorBodyKind,
) -> proc_macro2::TokenStream {
    let receiver = quote! { __statum_persisted };
    let emission_context = ValidatorCheckContext {
        machine_ident: context.machine_ident,
        machine_module_ident: context.machine_module_ident,
        machine_generics: context.machine_generics,
        field_names: context.field_names,
        receiver: &receiver,
    };
    let macro_suffix = match body_kind {
        ValidatorBodyKind::Build => "build",
        ValidatorBodyKind::Report => "report",
    };
    let emit_variant_macro_ident = format_ident!(
        "__statum_emit_{}_validator_{}_variant",
        crate::to_snake_case(context.machine_name),
        macro_suffix
    );
    let emit_contract_macro_ident = format_ident!(
        "__statum_emit_{}_validator_{}_variants",
        crate::to_snake_case(context.machine_name),
        macro_suffix
    );

    let method_arms = validator_methods.iter().map(|method| {
        let validator_ident = &method.validator_ident;
        let actual_ok_type = &method.actual_ok_type;
        let payload_check_fn_ident = format_ident!("__statum_payload_for_{}", validator_ident);
        let with_data_tokens = match body_kind {
            ValidatorBodyKind::Build => generate_validator_check_template(
                &emission_context,
                validator_ident,
                true,
                method.is_async,
            ),
            ValidatorBodyKind::Report => generate_validator_report_check_template(
                &emission_context,
                validator_ident,
                true,
                method.return_kind,
                method.is_async,
            ),
        };
        let without_data_tokens = match body_kind {
            ValidatorBodyKind::Build => generate_validator_check_template(
                &emission_context,
                validator_ident,
                false,
                method.is_async,
            ),
            ValidatorBodyKind::Report => generate_validator_report_check_template(
                &emission_context,
                validator_ident,
                false,
                method.return_kind,
                method.is_async,
            ),
        };

        quote! {
            (
                machine = $machine:ident,
                state_family = $state_family:ident,
                variant = $variant:ident,
                validator = #validator_ident,
                data = $data:ty,
                has_data = true,
            ) => {
                {
                    fn #payload_check_fn_ident(_: core::option::Option<$data>) {}
                    #payload_check_fn_ident(core::option::Option::<#actual_ok_type>::None);
                }
                #with_data_tokens
            };
            (
                machine = $machine:ident,
                state_family = $state_family:ident,
                variant = $variant:ident,
                validator = #validator_ident,
                data = $data:ty,
                has_data = false,
            ) => {
                {
                    fn #payload_check_fn_ident(_: core::option::Option<$data>) {}
                    #payload_check_fn_ident(core::option::Option::<#actual_ok_type>::None);
                }
                #without_data_tokens
            };
        }
    });

    quote! {
        #[doc(hidden)]
        macro_rules! #emit_variant_macro_ident {
            #(#method_arms)*
            (
                machine = $machine:ident,
                state_family = $state_family:ident,
                variant = $variant:ident,
                validator = $validator:ident,
                data = $data:ty,
                has_data = $has_data:tt,
            ) => {};
        }

        #[doc(hidden)]
        macro_rules! #emit_contract_macro_ident {
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
                            data = $data:ty,
                            has_data = $has_data:tt
                        }
                    ),* $(,)?
                ],
                $($rest:tt)*
            ) => {
                $(
                    #emit_variant_macro_ident!(
                        machine = $machine,
                        state_family = $state_family,
                        variant = $variant,
                        validator = $validator,
                        data = $data,
                        has_data = $has_data,
                    );
                )*
            };
        }

        #machine_validator_contract_macro_ident!(#emit_contract_macro_ident);
    }
}

fn generate_into_machine_builder(context: IntoMachineBuilderContext<'_>) -> proc_macro2::TokenStream {
    let builder_ident = context.builder_ident;
    let struct_ident = context.struct_ident;
    let machine_generics = context.machine_generics;
    let machine_state_ty = context.machine_state_ty;
    let field_names = context.field_names;
    let field_types = context.field_types;
    let validator_checks = context.validator_checks;
    let validator_report_checks = context.validator_report_checks;
    let validator_report_count = validator_report_checks.len();
    let async_token = context.async_token;
    let machine_vis = context.machine_vis;
    let extra_machine_generics = extra_generics(machine_generics);
    let slot_state_idents = (0..field_names.len())
        .map(|idx| format_ident!("__STATUM_SLOT_{}_SET", idx))
        .collect::<Vec<_>>();
    let builder_defaults = builder_generics(&extra_machine_generics, true, &slot_state_idents, true);
    let builder_impl_generics_decl =
        builder_generics(&extra_machine_generics, true, &slot_state_idents, false);
    let (builder_impl_generics, builder_ty_generics, builder_where_clause) =
        builder_impl_generics_decl.split_for_impl();
    let complete_slots = slot_state_idents
        .iter()
        .map(|_| quote! { true })
        .collect::<Vec<_>>();
    let complete_builder_ty_generics = generic_argument_tokens(
        extra_machine_generics.params.iter(),
        Some(quote! { '__statum_row }),
        &complete_slots,
    );
    let complete_builder_impl_generics_decl =
        builder_generics(&extra_machine_generics, true, &[], false);
    let (complete_builder_impl_generics, _, complete_builder_where_clause) =
        complete_builder_impl_generics_decl.split_for_impl();

    let struct_fields = field_names
        .iter()
        .zip(field_types.iter())
        .map(|(field_name, field_type)| {
            quote! { #field_name: core::option::Option<#field_type> }
        })
        .collect::<Vec<_>>();
    let field_bindings = field_names
        .iter()
        .map(|field_name| {
            let message = format!("statum internal error: `{field_name}` was not set before build");
            quote! {
                let #field_name = self.#field_name.expect(#message);
            }
        })
        .collect::<Vec<_>>();
    let setters = field_names
        .iter()
        .zip(field_types.iter())
        .enumerate()
        .map(|(slot_idx, (field_name, field_type))| {
            let generics = slot_state_idents
                .iter()
                .enumerate()
                .map(|(idx, ident)| {
                    if idx == slot_idx {
                        quote! { true }
                    } else {
                        quote! { #ident }
                    }
                })
                .collect::<Vec<_>>();
            let target_generics = generic_argument_tokens(
                extra_machine_generics.params.iter(),
                Some(quote! { '__statum_row }),
                &generics,
            );
            let assignments = field_names.iter().enumerate().map(|(idx, existing_field_name)| {
                if idx == slot_idx {
                    quote! { #existing_field_name: core::option::Option::Some(value) }
                } else {
                    quote! { #existing_field_name: self.#existing_field_name }
                }
            });

            quote! {
                #machine_vis fn #field_name(self, value: #field_type) -> #builder_ident #target_generics {
                    #builder_ident {
                        __statum_item: self.__statum_item,
                        #(#assignments),*
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    quote! {
        #[doc(hidden)]
        #machine_vis struct #builder_ident #builder_defaults {
            __statum_item: &'__statum_row #struct_ident,
            #(#struct_fields),*
        }

        impl #builder_impl_generics #builder_ident #builder_ty_generics #builder_where_clause {
            #(#setters)*
        }

        impl #complete_builder_impl_generics #builder_ident #complete_builder_ty_generics #complete_builder_where_clause {
            #machine_vis #async_token fn build(self) -> core::result::Result<#machine_state_ty, statum::Error> {
                let __statum_persisted = self.__statum_item;
                #(#field_bindings)*
                #(#validator_checks)*

                Err(statum::Error::InvalidState)
            }

            #machine_vis #async_token fn build_report(self) -> statum::RebuildReport<#machine_state_ty> {
                let __statum_persisted = self.__statum_item;
                let mut __statum_attempts = ::std::vec::Vec::with_capacity(#validator_report_count);
                #(#field_bindings)*
                #(#validator_report_checks)*

                statum::RebuildReport {
                    attempts: __statum_attempts,
                    result: Err(statum::Error::InvalidState),
                }
            }
        }
    }
}
