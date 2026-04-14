use super::contract::build_transition_contract;
use super::diagnostics::{
    compile_error_at, invalid_transition_method_state_error, invalid_transition_state_error,
};
use super::parse::{TransitionFn, TransitionImpl, strip_present_attrs_from_transition_impl};
use crate::machine::{
    to_shouty_snake_identifier, transition_presentation_slice_ident, transition_slice_ident,
    transition_support_module_ident,
};
use crate::{MachineInfo, PresentationAttr, PresentationTypesAttr, to_snake_case};
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::{GenericArgument, ItemImpl, LitStr, PathArguments, Type};

pub fn generate_transition_impl(
    input: &ItemImpl,
    tr_impl: &TransitionImpl,
    target_machine_info: &MachineInfo,
) -> TokenStream {
    let target_type = &tr_impl.target_type;
    let (impl_generics, _, where_clause) = input.generics.split_for_impl();
    let machine_target_ident = format_ident!("{}", target_machine_info.name);
    let transition_support_module_ident = transition_support_module_ident(target_machine_info);
    let field_names = target_machine_info.field_names();
    let machine_module_ident = format_ident!("{}", to_snake_case(&target_machine_info.name));
    let transition_slice_ident = transition_slice_ident(
        &target_machine_info.name,
        target_machine_info.file_path.as_deref(),
        target_machine_info.line_number,
    );
    let transition_presentation_slice_ident = transition_presentation_slice_ident(
        &target_machine_info.name,
        target_machine_info.file_path.as_deref(),
        target_machine_info.line_number,
    );
    let state_enum_info = match target_machine_info.get_matching_state_enum() {
        Ok(enum_info) => enum_info,
        Err(err) => return err,
    };
    let Some(source_variant_info) = state_enum_info.get_variant_from_name(&tr_impl.source_state)
    else {
        return invalid_transition_state_error(
            tr_impl.source_state_span,
            &tr_impl.machine_name,
            &tr_impl.source_state,
            &state_enum_info,
            "source",
        );
    };
    let source_data_ty = match source_variant_info.parse_data_type() {
        Ok(Some(data_ty)) => quote! { #data_ty },
        Ok(None) => quote! { () },
        Err(err) => return err,
    };
    let extra_transition_trait_args = match extra_machine_generic_argument_tokens(target_type) {
        Ok(args) => args,
        Err(err) => return err,
    };

    let mut emitted_states = HashSet::new();
    let mut transition_impls = Vec::new();
    for function in &tr_impl.functions {
        let contract = match build_transition_contract(function, target_type) {
            Ok(contract) => contract,
            Err(err) => return err,
        };

        for return_state in contract.next_states {
            if !emitted_states.insert(return_state.clone()) {
                continue;
            }
            let return_state_ident = format_ident!("{}", return_state);
            let next_machine_ty = match replace_machine_state_in_target_type(
                target_type,
                syn::parse_quote!(#return_state_ident),
            ) {
                Ok(ty) => ty,
                Err(err) => return err,
            };
            let Some(variant_info) = state_enum_info.get_variant_from_name(&return_state) else {
                return invalid_transition_method_state_error(
                    function,
                    &tr_impl.machine_name,
                    &return_state,
                    &state_enum_info,
                );
            };

            transition_impls.push(match variant_info.parse_data_type() {
                Ok(Some(data_ty)) => {
                    quote! {
                        impl #impl_generics #transition_support_module_ident::TransitionWith<#data_ty #extra_transition_trait_args> for #target_type #where_clause {
                            type NextState = #return_state_ident;
                            fn transition_with(self, data: #data_ty) -> #next_machine_ty {
                                #machine_target_ident {
                                    marker: core::marker::PhantomData,
                                    state_data: data,
                                    #(#field_names: self.#field_names,)*
                                }
                            }
                        }

                        impl #impl_generics statum::CanTransitionWith<#data_ty> for #target_type #where_clause {
                            type NextState = #return_state_ident;
                            type Output = #next_machine_ty;

                            fn transition_with_data(self, data: #data_ty) -> Self::Output {
                                <Self as #transition_support_module_ident::TransitionWith<#data_ty #extra_transition_trait_args>>::transition_with(self, data)
                            }
                        }

                        impl #impl_generics #transition_support_module_ident::DeclaredTransitionMapEdge<#return_state_ident #extra_transition_trait_args> for #target_type #where_clause {
                            type CurrentData = #source_data_ty;

                            fn transition_map<F>(self, f: F) -> #next_machine_ty
                            where
                                F: FnOnce(Self::CurrentData) -> <#return_state_ident as statum::StateMarker>::Data,
                            {
                                let Self {
                                    marker: _,
                                    state_data,
                                    #(#field_names),*
                                } = self;

                                #machine_target_ident {
                                    marker: core::marker::PhantomData,
                                    state_data: f(state_data),
                                    #(#field_names),*
                                }
                            }
                        }

                        impl #impl_generics statum::CanTransitionMap<#return_state_ident> for #target_type #where_clause {
                            type CurrentData = <Self as #transition_support_module_ident::DeclaredTransitionMapEdge<#return_state_ident #extra_transition_trait_args>>::CurrentData;
                            type Output = #next_machine_ty;

                            fn transition_map<F>(self, f: F) -> Self::Output
                            where
                                F: FnOnce(Self::CurrentData) -> <#return_state_ident as statum::StateMarker>::Data,
                            {
                                <Self as #transition_support_module_ident::DeclaredTransitionMapEdge<#return_state_ident #extra_transition_trait_args>>::transition_map(self, f)
                            }
                        }
                    }
                }
                Ok(None) => {
                    quote! {
                        impl #impl_generics #transition_support_module_ident::TransitionTo<#return_state_ident #extra_transition_trait_args> for #target_type #where_clause {
                            fn transition(self) -> #next_machine_ty {
                                #machine_target_ident {
                                    marker: core::marker::PhantomData,
                                    state_data: (),
                                    #(#field_names: self.#field_names,)*
                                }
                            }
                        }

                        impl #impl_generics statum::CanTransitionTo<#return_state_ident> for #target_type #where_clause {
                            type Output = #next_machine_ty;

                            fn transition_to(self) -> Self::Output {
                                <Self as #transition_support_module_ident::TransitionTo<#return_state_ident #extra_transition_trait_args>>::transition(self)
                            }
                        }
                    }
                }
                Err(err) => return err,
            });
        }
    }
    let transition_registrations = tr_impl.functions.iter().enumerate().map(|(idx, function)| {
        let contract = match build_transition_contract(function, target_type) {
            Ok(contract) => contract,
            Err(err) => return err,
        };
        let return_states = contract.next_states;
        let unique_suffix = transition_site_unique_suffix(tr_impl, function, idx);
        let token_ident = format_ident!("__STATUM_TRANSITION_TOKEN_{}", unique_suffix);
        let targets_ident = format_ident!("__STATUM_TRANSITION_TARGETS_{}", unique_suffix);
        let registration_ident = format_ident!("__STATUM_TRANSITION_SITE_{}", unique_suffix);
        let id_const_ident = format_ident!(
            "{}",
            to_shouty_snake_identifier(&function.name.to_string())
        );
        let method_name = LitStr::new(&function.name.to_string(), function.name.span());
        let source_state_ident = format_ident!("{}", tr_impl.source_state);
        let target_state_idents = return_states.iter().map(|state| {
            let state_ident = format_ident!("{}", state);
            quote! { #machine_module_ident::StateId::#state_ident }
        });
        let target_state_count = return_states.len();
        let cfg_attrs = propagated_cfg_attrs(&tr_impl.attrs, &function.attrs);

        quote! {
            #(#cfg_attrs)*
            static #targets_ident: [#machine_module_ident::StateId; #target_state_count] = [
                #(#target_state_idents),*
            ];

            #(#cfg_attrs)*
            static #token_ident: statum::__private::TransitionToken =
                statum::__private::TransitionToken::new();

            #(#cfg_attrs)*
            #[statum::__private::linkme::distributed_slice(#machine_module_ident::#transition_slice_ident)]
            #[linkme(crate = statum::__private::linkme)]
            static #registration_ident:
                statum::TransitionDescriptor<#machine_module_ident::StateId, #machine_module_ident::TransitionId> =
                statum::TransitionDescriptor {
                    id: #machine_module_ident::TransitionId::from_token(&#token_ident),
                    method_name: #method_name,
                    from: #machine_module_ident::StateId::#source_state_ident,
                    to: &#targets_ident,
                };

            #(#cfg_attrs)*
            impl #impl_generics #target_type #where_clause {
                pub const #id_const_ident: #machine_module_ident::TransitionId =
                    #machine_module_ident::TransitionId::from_token(&#token_ident);
            }
        }
    });
    let transition_presentation_registrations = tr_impl
        .functions
        .iter()
        .enumerate()
        .filter_map(|(idx, function)| {
            let presentation = function.presentation.as_ref()?;
            let unique_suffix = transition_site_unique_suffix(tr_impl, function, idx);
            let token_ident = format_ident!("__STATUM_TRANSITION_TOKEN_{}", unique_suffix);
            let registration_ident =
                format_ident!("__STATUM_TRANSITION_PRESENTATION_{}", unique_suffix);
            let label =
                optional_lit_str_tokens(presentation.label.as_deref(), function.name.span());
            let description =
                optional_lit_str_tokens(presentation.description.as_deref(), function.name.span());
            let transition_meta_ty = match transition_presentation_type_tokens(target_machine_info) {
                Ok(tokens) => tokens,
                Err(err) => return Some(err),
            };
            let metadata = match transition_presentation_metadata_tokens(
                presentation,
                function,
                &target_machine_info.name,
                target_machine_info,
            ) {
                Ok(tokens) => tokens,
                Err(err) => return Some(err),
            };
            let cfg_attrs = propagated_cfg_attrs(&tr_impl.attrs, &function.attrs);

            Some(quote! {
                #(#cfg_attrs)*
                #[statum::__private::linkme::distributed_slice(#machine_module_ident::#transition_presentation_slice_ident)]
                #[linkme(crate = statum::__private::linkme)]
                static #registration_ident:
                    statum::__private::TransitionPresentation<#machine_module_ident::TransitionId, #transition_meta_ty> =
                    statum::__private::TransitionPresentation {
                        id: #machine_module_ident::TransitionId::from_token(&#token_ident),
                        label: #label,
                        description: #description,
                        metadata: #metadata,
                    };
            })
        });
    let sanitized_input = strip_present_attrs_from_transition_impl(input);

    quote! {
        #(#transition_impls)*
        #(#transition_registrations)*
        #(#transition_presentation_registrations)*
        #sanitized_input
    }
}

fn optional_lit_str_tokens(value: Option<&str>, span: Span) -> TokenStream {
    match value {
        Some(value) => {
            let lit = LitStr::new(value, span);
            quote! { Some(#lit) }
        }
        None => quote! { None },
    }
}

fn transition_presentation_type_tokens(
    machine_info: &MachineInfo,
) -> Result<TokenStream, TokenStream> {
    let Some(presentation_types) = machine_info.presentation_types.as_ref() else {
        return Ok(quote! { () });
    };
    let Some(transition_ty) = presentation_types
        .parse_transition_type()
        .map_err(|err| err.to_compile_error())?
    else {
        return Ok(quote! { () });
    };

    Ok(quote! { #transition_ty })
}

fn transition_presentation_metadata_tokens(
    presentation: &PresentationAttr,
    function: &TransitionFn,
    machine_name: &str,
    machine_info: &MachineInfo,
) -> Result<TokenStream, TokenStream> {
    let transition_metadata_ty = machine_info
        .presentation_types
        .as_ref()
        .map(PresentationTypesAttr::parse_transition_type)
        .transpose()
        .map_err(|err| err.to_compile_error())?
        .flatten();

    match (presentation.metadata.as_deref(), transition_metadata_ty) {
        (Some(metadata_expr), Some(_)) => {
            let metadata =
                syn::parse_str::<syn::Expr>(metadata_expr).map_err(|err| err.to_compile_error())?;
            Ok(quote! { #metadata })
        }
        (Some(_), None) => Err(compile_error_at(
            function.name.span(),
            &format!(
                "Error: transition `{}::{}` uses `#[present(metadata = ...)]`, but machine `{machine_name}` did not declare `#[presentation_types(transition = ...)]`.\nFix: add `#[presentation_types(transition = TransitionMeta)]` to `{machine_name}` or remove the metadata expression.",
                tr_impl_machine_state_display(machine_name, &function.source_state),
                function.name,
            ),
        )),
        (None, Some(_)) => Err(compile_error_at(
            function.name.span(),
            &format!(
                "Error: transition `{}::{}` uses `#[present(...)]`, and machine `{machine_name}` declared `#[presentation_types(transition = ...)]`.\nFix: add `metadata = ...` to that transition so the generated typed presentation surface has a value for every annotated transition.",
                tr_impl_machine_state_display(machine_name, &function.source_state),
                function.name,
            ),
        )),
        _ => Ok(quote! { () }),
    }
}

fn tr_impl_machine_state_display(machine_name: &str, source_state: &str) -> String {
    format!("{machine_name}<{}>", source_state)
}

fn replace_machine_state_in_target_type(
    target_type: &Type,
    next_state: Type,
) -> Result<Type, TokenStream> {
    let mut replaced = target_type.clone();
    let Type::Path(type_path) = &mut replaced else {
        return Err(compile_error_at(
            target_type.span(),
            "Invalid #[transition] target type. Expected an impl target like `Machine<State>`.",
        ));
    };
    let Some(segment) = type_path.path.segments.last_mut() else {
        return Err(compile_error_at(
            target_type.span(),
            "Invalid #[transition] target type. Expected an impl target like `Machine<State>`.",
        ));
    };
    let PathArguments::AngleBracketed(args) = &mut segment.arguments else {
        return Err(compile_error_at(
            target_type.span(),
            "Invalid #[transition] target type. Expected an impl target like `Machine<State>`.",
        ));
    };

    let mut replaced_args = syn::punctuated::Punctuated::new();
    replaced_args.push(GenericArgument::Type(next_state));
    for argument in args.args.iter().skip(1) {
        replaced_args.push(argument.clone());
    }
    args.args = replaced_args;

    Ok(replaced)
}

fn extra_machine_generic_argument_tokens(target_type: &Type) -> Result<TokenStream, TokenStream> {
    let Type::Path(type_path) = target_type else {
        return Err(compile_error_at(
            target_type.span(),
            "Invalid #[transition] target type. Expected an impl target like `Machine<State>`.",
        ));
    };
    let Some(segment) = type_path.path.segments.last() else {
        return Err(compile_error_at(
            target_type.span(),
            "Invalid #[transition] target type. Expected an impl target like `Machine<State>`.",
        ));
    };
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Err(compile_error_at(
            target_type.span(),
            "Invalid #[transition] target type. Expected an impl target like `Machine<State>`.",
        ));
    };

    let extra_args = args.args.iter().skip(1).collect::<Vec<_>>();
    if extra_args.is_empty() {
        Ok(quote! {})
    } else {
        Ok(quote! {, #(#extra_args),* })
    }
}

fn propagated_cfg_attrs(
    impl_attrs: &[syn::Attribute],
    function_attrs: &[syn::Attribute],
) -> Vec<syn::Attribute> {
    impl_attrs
        .iter()
        .chain(function_attrs.iter())
        .filter(|attr| {
            attr.path()
                .get_ident()
                .is_some_and(|ident| ident == "cfg" || ident == "cfg_attr")
        })
        .cloned()
        .collect()
}

fn transition_site_unique_suffix(
    tr_impl: &TransitionImpl,
    function: &TransitionFn,
    index: usize,
) -> String {
    let attrs = function
        .attrs
        .iter()
        .map(|attr| attr.to_token_stream().to_string())
        .collect::<Vec<_>>()
        .join("|");
    let return_type = function
        .return_type
        .as_ref()
        .map(|ty| ty.to_token_stream().to_string())
        .unwrap_or_default();
    let signature = format!(
        "{}::{}::{}::{}::{}::{}",
        tr_impl.machine_name, tr_impl.source_state, function.name, index, attrs, return_type,
    );

    format!("{:016x}", stable_hash(&signature))
}

fn stable_hash(input: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
