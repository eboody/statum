use quote::{format_ident, quote};
use syn::{GenericParam, Generics, Ident, ImplItem, ImplItemFn, Type};

use crate::machine::{
    builder_generics, extra_generics, extra_type_arguments_tokens, generic_argument_tokens,
};
use crate::validators::signatures::ValidatorReturnKind;

pub(super) struct BatchBuilderContext<'a> {
    pub(super) machine_ident: &'a Ident,
    pub(super) machine_module_ident: &'a Ident,
    pub(super) machine_generics: &'a Generics,
    pub(super) struct_ident: &'a Type,
    pub(super) machine_state_ty: &'a proc_macro2::TokenStream,
    pub(super) field_names: &'a [Ident],
    pub(super) field_types: &'a [Type],
    pub(super) async_token: proc_macro2::TokenStream,
    pub(super) machine_vis: syn::Visibility,
}

pub(super) struct ValidatorCheckContext<'a> {
    pub(super) machine_ident: &'a Ident,
    pub(super) machine_module_ident: &'a Ident,
    pub(super) machine_generics: &'a Generics,
    pub(super) field_names: &'a [Ident],
    pub(super) receiver: &'a proc_macro2::TokenStream,
}

pub(super) fn generate_validator_check_template(
    context: &ValidatorCheckContext<'_>,
    validator_fn_ident: &Ident,
    has_state_data: bool,
    is_async: bool,
) -> proc_macro2::TokenStream {
    let receiver = context.receiver;
    let field_names = context.field_names;
    let await_token = if is_async {
        quote! { .await }
    } else {
        quote! {}
    };
    let field_builder_chain = quote! { #(.#field_names(#field_names))* };
    let machine_builder_path =
        machine_builder_path_template_tokens(context.machine_ident, context.machine_generics);
    let machine_state_variant_path = machine_state_variant_path_template_tokens(
        context.machine_module_ident,
        context.machine_generics,
    );

    if has_state_data {
        let builder_call = quote! {
            #machine_builder_path::builder()
                #field_builder_chain
                .state_data(__statum_state_data)
                .build()
        };
        quote! {
            if let Ok(__statum_state_data) = #receiver.#validator_fn_ident(#(&#field_names),*)#await_token {
                return Ok(#machine_state_variant_path(
                    #builder_call
                ));
            }
        }
    } else {
        let builder_call = quote! {
            #machine_builder_path::builder()
                #field_builder_chain
                .build()
        };
        quote! {
            if #receiver.#validator_fn_ident(#(&#field_names),*)#await_token.is_ok() {
                return Ok(#machine_state_variant_path(
                    #builder_call
                ));
            }
        }
    }
}

pub(super) fn generate_validator_report_check_template(
    context: &ValidatorCheckContext<'_>,
    validator_fn_ident: &Ident,
    has_state_data: bool,
    return_kind: ValidatorReturnKind,
    is_async: bool,
) -> proc_macro2::TokenStream {
    let receiver = context.receiver;
    let field_names = context.field_names;
    let await_token = if is_async {
        quote! { .await }
    } else {
        quote! {}
    };
    let field_builder_chain = quote! { #(.#field_names(#field_names))* };
    let matched_attempt = rebuild_attempt_template_tokens(validator_fn_ident, true);
    let failed_attempt = rebuild_attempt_template_tokens(validator_fn_ident, false);
    let failed_attempt_with_rejection =
        failed_rebuild_attempt_with_rejection_template_tokens(validator_fn_ident);
    let machine_builder_path =
        machine_builder_path_template_tokens(context.machine_ident, context.machine_generics);
    let machine_state_variant_path = machine_state_variant_path_template_tokens(
        context.machine_module_ident,
        context.machine_generics,
    );

    if has_state_data {
        let builder_call = quote! {
            #machine_builder_path::builder()
                #field_builder_chain
                .state_data(__statum_state_data)
                .build()
        };
        match return_kind {
            ValidatorReturnKind::Plain => quote! {
                match #receiver.#validator_fn_ident(#(&#field_names),*)#await_token {
                    Ok(__statum_state_data) => {
                        __statum_attempts.push(#matched_attempt);
                        return statum::RebuildReport {
                            attempts: __statum_attempts,
                            result: Ok(#machine_state_variant_path(#builder_call)),
                        };
                    }
                    Err(_) => __statum_attempts.push(#failed_attempt),
                }
            },
            ValidatorReturnKind::Diagnostic => quote! {
                match #receiver.#validator_fn_ident(#(&#field_names),*)#await_token {
                    Ok(__statum_state_data) => {
                        __statum_attempts.push(#matched_attempt);
                        return statum::RebuildReport {
                            attempts: __statum_attempts,
                            result: Ok(#machine_state_variant_path(#builder_call)),
                        };
                    }
                    Err(__statum_rejection) => __statum_attempts.push(#failed_attempt_with_rejection),
                }
            },
        }
    } else {
        let builder_call = quote! {
            #machine_builder_path::builder()
                #field_builder_chain
                .build()
        };
        match return_kind {
            ValidatorReturnKind::Plain => quote! {
                if #receiver.#validator_fn_ident(#(&#field_names),*)#await_token.is_ok() {
                    __statum_attempts.push(#matched_attempt);
                    return statum::RebuildReport {
                        attempts: __statum_attempts,
                        result: Ok(#machine_state_variant_path(#builder_call)),
                    };
                }

                __statum_attempts.push(#failed_attempt);
            },
            ValidatorReturnKind::Diagnostic => quote! {
                match #receiver.#validator_fn_ident(#(&#field_names),*)#await_token {
                    Ok(()) => {
                        __statum_attempts.push(#matched_attempt);
                        return statum::RebuildReport {
                            attempts: __statum_attempts,
                            result: Ok(#machine_state_variant_path(#builder_call)),
                        };
                    }
                    Err(__statum_rejection) => {
                        __statum_attempts.push(#failed_attempt_with_rejection);
                    }
                }
            },
        }
    }
}

pub(super) fn batch_builder_implementation(
    context: BatchBuilderContext<'_>,
) -> proc_macro2::TokenStream {
    let builder_ident = format_ident!("__Statum{}IntoMachines", context.machine_ident);
    let by_builder_ident = format_ident!("__Statum{}IntoMachinesBy", context.machine_ident);
    let machine_module_ident = context.machine_module_ident;
    let machine_generics = context.machine_generics;
    let struct_ident = context.struct_ident;
    let machine_state_ty = context.machine_state_ty;
    let field_names = context.field_names;
    let field_types = context.field_types;
    let async_token = context.async_token;
    let machine_vis = context.machine_vis;
    let extra_machine_generics = extra_generics(machine_generics);
    let extra_machine_ty_args = extra_type_arguments_tokens(machine_generics);
    let fields_ty = quote! { #machine_module_ident::Fields #extra_machine_ty_args };
    let extra_impl_params = extra_machine_generics
        .params
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let extra_trait_args = trait_extra_generic_argument_tokens(&extra_machine_generics);
    let into_machines_impl_generics = if extra_impl_params.is_empty() {
        quote! { <T> }
    } else {
        quote! { <T, #(#extra_impl_params),*> }
    };
    let into_machines_where_clause = merged_where_clause_tokens(
        extra_machine_generics.where_clause.as_ref(),
        vec![quote! { T: Into<Vec<#struct_ident>> }],
    );

    let field_builder_chain = quote! { #(.#field_names(#field_names.clone()))* };
    let per_item_builder_chain = quote! { #(.#field_names(__statum_fields.#field_names))* };
    let await_token = if async_token.is_empty() {
        quote! {}
    } else {
        quote! { .await }
    };

    let implementation = generate_finalization_logic(
        &format_ident!("build"),
        &field_builder_chain,
        &async_token,
    );
    let report_implementation = generate_finalization_logic(
        &format_ident!("build_report"),
        &field_builder_chain,
        &async_token,
    );
    let per_item_implementation =
        generate_per_item_finalization_logic(
            &format_ident!("build"),
            &per_item_builder_chain,
            &async_token,
        );
    let per_item_report_implementation =
        generate_per_item_finalization_logic(
            &format_ident!("build_report"),
            &per_item_builder_chain,
            &async_token,
        );
    let slot_state_idents = (0..field_names.len())
        .map(|idx| format_ident!("__STATUM_SLOT_{}_SET", idx))
        .collect::<Vec<_>>();
    let builder_defaults = builder_generics(&extra_machine_generics, false, &slot_state_idents, true);
    let builder_impl_generics_decl =
        builder_generics(&extra_machine_generics, false, &slot_state_idents, false);
    let (builder_impl_generics, builder_ty_generics, builder_where_clause) =
        builder_impl_generics_decl.split_for_impl();
    let initial_builder_slots = slot_state_idents
        .iter()
        .map(|_| quote! { false })
        .collect::<Vec<_>>();
    let initial_builder_ty_generics =
        generic_argument_tokens(extra_machine_generics.params.iter(), None, &initial_builder_slots);
    let complete_builder_slots = slot_state_idents
        .iter()
        .map(|_| quote! { true })
        .collect::<Vec<_>>();
    let complete_builder_ty_generics =
        generic_argument_tokens(extra_machine_generics.params.iter(), None, &complete_builder_slots);
    let complete_builder_impl_generics_decl =
        builder_generics(&extra_machine_generics, false, &[], false);
    let (complete_builder_impl_generics, _, complete_builder_where_clause) =
        complete_builder_impl_generics_decl.split_for_impl();
    let shared_builder_where_clause = merged_where_clause_tokens(
        complete_builder_where_clause,
        field_types
            .iter()
            .map(|field_type| quote! { #field_type: core::clone::Clone })
            .collect(),
    );
    let by_builder_decl_generics = prefixed_generics_declaration_tokens("F", &extra_machine_generics);
    let by_builder_ty_generics =
        prefixed_generics_argument_tokens(quote! { F }, extra_machine_generics.params.iter());
    let by_builder_where_clause = merged_where_clause_tokens(
        extra_machine_generics.where_clause.as_ref(),
        vec![quote! { F: Fn(&#struct_ident) -> #fields_ty }],
    );
    let by_builder_marker_field = if extra_machine_generics.params.is_empty() {
        quote! {}
    } else {
        let marker_ty = generic_usage_marker_tokens(&extra_machine_generics);
        quote! {
            __statum_marker: core::marker::PhantomData<fn() -> #marker_ty>,
        }
    };
    let by_builder_marker_init = if extra_machine_generics.params.is_empty() {
        quote! {}
    } else {
        quote! {
            __statum_marker: core::marker::PhantomData,
        }
    };
    let field_storage = field_names.iter().zip(field_types.iter()).map(|(field_name, field_type)| {
        quote! { #field_name: core::option::Option<#field_type> }
    });
    let builder_init = field_names.iter().map(|field_name| {
        quote! { #field_name: core::option::Option::None }
    });
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
            let target_generics =
                generic_argument_tokens(extra_machine_generics.params.iter(), None, &generics);
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
                        __statum_items: self.__statum_items,
                        #(#assignments),*
                    }
                }
            }
        });

    quote! {
        impl #into_machines_impl_generics #machine_module_ident::IntoMachinesExt<#struct_ident #extra_trait_args> for T
        #into_machines_where_clause
        {
            type Builder = #builder_ident #initial_builder_ty_generics;
            type BuilderWithFields<F> = #by_builder_ident #by_builder_ty_generics;

            fn into_machines(self) -> Self::Builder {
                #builder_ident {
                    __statum_items: self.into(),
                    #(#builder_init),*
                }
            }

            fn into_machines_by<F>(self, fields: F) -> Self::BuilderWithFields<F>
            where
                F: Fn(&#struct_ident) -> #fields_ty,
            {
                #by_builder_ident {
                    __statum_items: self.into(),
                    __statum_fields_fn: fields,
                    #by_builder_marker_init
                }
            }
        }

        #[doc(hidden)]
        #machine_vis struct #builder_ident #builder_defaults {
            __statum_items: Vec<#struct_ident>,
            #(#field_storage),*
        }

        impl #builder_impl_generics #builder_ident #builder_ty_generics #builder_where_clause {
            #(#setters)*
        }

        impl #complete_builder_impl_generics #builder_ident #complete_builder_ty_generics #shared_builder_where_clause {
            #[inline(always)]
            #machine_vis #async_token fn build(self) -> Vec<core::result::Result<#machine_state_ty, statum::Error>> {
                let __statum_items = self.__statum_items;
                #(#field_bindings)*
                #implementation
            }

            #[inline(always)]
            #machine_vis #async_token fn build_reports(self) -> Vec<statum::RebuildReport<#machine_state_ty>> {
                let __statum_items = self.__statum_items;
                #(#field_bindings)*
                #report_implementation
            }
        }

        #[doc(hidden)]
        #machine_vis struct #by_builder_ident #by_builder_decl_generics {
            __statum_items: Vec<#struct_ident>,
            __statum_fields_fn: F,
            #by_builder_marker_field
        }

        impl #by_builder_decl_generics #by_builder_ident #by_builder_ty_generics
        #by_builder_where_clause
        {
            #[inline(always)]
            #machine_vis #async_token fn build(self) -> Vec<core::result::Result<#machine_state_ty, statum::Error>> {
                self.__private_finalize()#await_token
            }

            #[inline(always)]
            #machine_vis #async_token fn build_reports(self) -> Vec<statum::RebuildReport<#machine_state_ty>> {
                self.__private_finalize_reports()#await_token
            }

            #async_token fn __private_finalize(self) -> Vec<core::result::Result<#machine_state_ty, statum::Error>> {
                #per_item_implementation
            }

            #async_token fn __private_finalize_reports(self) -> Vec<statum::RebuildReport<#machine_state_ty>> {
                #per_item_report_implementation
            }
        }
    }
}

fn generate_finalization_logic(
    builder_method: &Ident,
    field_builder_chain: &proc_macro2::TokenStream,
    async_token: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    if async_token.is_empty() {
        quote! {
            __statum_items
                .into_iter()
                .map(|__statum_item| {
                    __statum_item.into_machine()
                        #field_builder_chain
                        .#builder_method()
                })
                .collect()
        }
    } else {
        quote! {
            statum::__private::futures::future::join_all(
                __statum_items.iter().map(|__statum_item| {
                    __statum_item.into_machine()
                        #field_builder_chain
                        .#builder_method()
                })
            ).await
        }
    }
}

fn generate_per_item_finalization_logic(
    builder_method: &Ident,
    field_builder_chain: &proc_macro2::TokenStream,
    async_token: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    if async_token.is_empty() {
        quote! {
            let __statum_field_fn = self.__statum_fields_fn;
            self.__statum_items
                .into_iter()
                .map(|__statum_item| {
                    let __statum_fields = __statum_field_fn(&__statum_item);
                    __statum_item.into_machine()
                        #field_builder_chain
                        .#builder_method()
                })
                .collect()
        }
    } else {
        quote! {
            let __statum_field_fn = &self.__statum_fields_fn;
            statum::__private::futures::future::join_all(
                self.__statum_items.iter().map(|__statum_item| {
                    let __statum_fields = __statum_field_fn(__statum_item);
                    __statum_item.into_machine()
                        #field_builder_chain
                        .#builder_method()
                })
            ).await
        }
    }
}

fn rebuild_attempt_template_tokens(
    validator_fn_ident: &Ident,
    matched: bool,
) -> proc_macro2::TokenStream {
    quote! {
        statum::RebuildAttempt {
            validator: stringify!(#validator_fn_ident),
            target_state: stringify!($variant),
            matched: #matched,
            reason_key: core::option::Option::None,
            message: core::option::Option::None,
        }
    }
}

fn failed_rebuild_attempt_with_rejection_template_tokens(
    validator_fn_ident: &Ident,
) -> proc_macro2::TokenStream {
    quote! {
        statum::RebuildAttempt {
            validator: stringify!(#validator_fn_ident),
            target_state: stringify!($variant),
            matched: false,
            reason_key: core::option::Option::Some(__statum_rejection.reason_key),
            message: __statum_rejection.message.clone(),
        }
    }
}

fn machine_builder_path_template_tokens(
    machine_ident: &Ident,
    machine_generics: &Generics,
) -> proc_macro2::TokenStream {
    let extra_args = machine_generics
        .params
        .iter()
        .skip(1)
        .map(generic_argument_token)
        .collect::<Vec<_>>();
    if extra_args.is_empty() {
        quote! { #machine_ident::<$variant> }
    } else {
        quote! { #machine_ident::<$variant, #(#extra_args),*> }
    }
}

fn machine_state_variant_path_template_tokens(
    machine_module_ident: &Ident,
    machine_generics: &Generics,
) -> proc_macro2::TokenStream {
    let extra_args = machine_generics
        .params
        .iter()
        .skip(1)
        .map(generic_argument_token)
        .collect::<Vec<_>>();
    if extra_args.is_empty() {
        quote! { #machine_module_ident::SomeState::$variant }
    } else {
        quote! { #machine_module_ident::SomeState::<#(#extra_args),*>::$variant }
    }
}

fn generic_usage_marker_tokens(generics: &Generics) -> proc_macro2::TokenStream {
    let usages = generics
        .params
        .iter()
        .map(|param| match param {
            GenericParam::Lifetime(lifetime) => {
                let lifetime = &lifetime.lifetime;
                quote! { &#lifetime () }
            }
            GenericParam::Type(ty) => {
                let ident = &ty.ident;
                quote! { #ident }
            }
            GenericParam::Const(const_param) => {
                let ident = &const_param.ident;
                quote! { [(); #ident] }
            }
        })
        .collect::<Vec<_>>();

    if usages.len() == 1 {
        usages.into_iter().next().unwrap()
    } else {
        quote! { (#(#usages),*) }
    }
}

fn trait_extra_generic_argument_tokens(extra_generics: &Generics) -> proc_macro2::TokenStream {
    let extra_args = extra_generics
        .params
        .iter()
        .map(generic_argument_token)
        .collect::<Vec<_>>();
    if extra_args.is_empty() {
        quote! {}
    } else {
        quote! {, #(#extra_args),* }
    }
}

fn prefixed_generics_declaration_tokens(
    first_param: &str,
    extra_generics: &Generics,
) -> proc_macro2::TokenStream {
    let first_ident = format_ident!("{}", first_param);
    let extra_params = extra_generics.params.iter().cloned().collect::<Vec<_>>();
    if extra_params.is_empty() {
        quote! { <#first_ident> }
    } else {
        quote! { <#first_ident, #(#extra_params),*> }
    }
}

fn prefixed_generics_argument_tokens<'a>(
    first_arg: proc_macro2::TokenStream,
    extra_params: impl Iterator<Item = &'a GenericParam>,
) -> proc_macro2::TokenStream {
    let mut args = vec![first_arg];
    args.extend(extra_params.map(generic_argument_token));
    quote! { <#(#args),*> }
}

fn merged_where_clause_tokens(
    extra_where_clause: Option<&syn::WhereClause>,
    additional_predicates: Vec<proc_macro2::TokenStream>,
) -> proc_macro2::TokenStream {
    let mut predicates = extra_where_clause
        .into_iter()
        .flat_map(|where_clause| where_clause.predicates.iter().map(|predicate| quote! { #predicate }))
        .collect::<Vec<_>>();
    predicates.extend(additional_predicates);

    if predicates.is_empty() {
        quote! {}
    } else {
        quote! { where #(#predicates),* }
    }
}

fn generic_argument_token(param: &GenericParam) -> proc_macro2::TokenStream {
    match param {
        GenericParam::Lifetime(lifetime) => {
            let lifetime = &lifetime.lifetime;
            quote! { #lifetime }
        }
        GenericParam::Type(ty) => {
            let ident = &ty.ident;
            quote! { #ident }
        }
        GenericParam::Const(const_param) => {
            let ident = &const_param.ident;
            quote! { #ident }
        }
    }
}

pub(super) fn inject_machine_fields(
    methods: &[ImplItem],
    parsed_fields: &[(Ident, Type)],
    extra_machine_generics: &Generics,
) -> Result<Vec<ImplItem>, proc_macro2::TokenStream> {
    Ok(methods
        .iter()
        .map(|item| {
            if let ImplItem::Fn(func) = item {
                let fn_name = &func.sig.ident;

                if super::signatures::validator_state_name_from_ident(fn_name).is_some() {
                    let mut new_inputs = func.sig.inputs.clone();

                    for (ident, ty) in parsed_fields.iter() {
                        new_inputs.push(syn::FnArg::Typed(syn::parse_quote! { #ident: &#ty }));
                    }

                    let mut generics = func.sig.generics.clone();
                    if !extra_machine_generics.params.is_empty() {
                        if generics.lt_token.is_none() {
                            generics.lt_token = Some(Default::default());
                            generics.gt_token = Some(Default::default());
                        }
                        generics
                            .params
                            .extend(extra_machine_generics.params.iter().cloned());
                    }
                    if let Some(extra_where_clause) = &extra_machine_generics.where_clause {
                        let where_clause = generics.make_where_clause();
                        where_clause
                            .predicates
                            .extend(extra_where_clause.predicates.iter().cloned());
                    }

                    let mut attrs = func.attrs.clone();
                    attrs.push(syn::parse_quote!(#[allow(clippy::ptr_arg)]));
                    let body = &func.block;

                    return ImplItem::Fn(ImplItemFn {
                        attrs,
                        sig: syn::Signature {
                            inputs: new_inputs,
                            generics,
                            ..func.sig.clone()
                        },
                        block: body.clone(),
                        ..func.clone()
                    });
                }
            }
            item.clone()
        })
        .collect())
}
