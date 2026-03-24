use macro_registry::callsite::current_source_info;
use macro_registry::query;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::{
    AngleBracketedGenericArguments, Block, FnArg, GenericArgument, Ident, ImplItem, ImplItemFn,
    ItemImpl, LitStr, PathArguments, ReturnType, Type, TypePath,
};

use crate::machine::{
    to_shouty_snake_identifier, transition_presentation_slice_ident, transition_slice_ident,
    transition_support_module_ident,
};
use crate::{
    EnumInfo, MachineInfo, PresentationAttr, PresentationTypesAttr, parse_present_attrs,
    strip_present_attrs, to_snake_case,
};

/// Stores all metadata for a single transition method in an `impl` block
#[allow(unused)]
pub struct TransitionFn {
    pub name: Ident,
    pub attrs: Vec<syn::Attribute>,
    pub presentation: Option<PresentationAttr>,
    pub has_receiver: bool,
    pub return_type: Option<Type>,
    pub return_type_span: Option<Span>,
    pub machine_name: String,
    pub source_state: String,
    pub generics: Vec<Ident>,
    pub internals: Block,
    pub is_async: bool,
    pub vis: syn::Visibility,
    pub span: proc_macro2::Span,
}

impl TransitionFn {
    pub fn return_state(&self, target_type: &Type) -> Result<String, TokenStream> {
        let Some(return_type) = self.return_type.as_ref() else {
            return Err(invalid_return_type_error(self, "missing return type"));
        };
        let Some((_, return_state)) = parse_machine_and_state(return_type, target_type) else {
            return Err(invalid_return_type_error(
                self,
                "expected the impl target machine path directly, or the same path wrapped in a canonical `::core::option::Option`, `::core::result::Result`, or `::statum::Branch`",
            ));
        };

        Ok(return_state)
    }

    pub fn return_states(&self, target_type: &Type) -> Result<Vec<String>, TokenStream> {
        let Some(return_type) = self.return_type.as_ref() else {
            return Err(invalid_return_type_error(self, "missing return type"));
        };
        let return_states = collect_machine_and_states(return_type, target_type)
            .into_iter()
            .map(|(_, state)| state)
            .collect::<Vec<_>>();
        if return_states.is_empty() {
            return Err(invalid_return_type_error(
                self,
                "expected the impl target machine path directly, or the same path wrapped in a canonical `::core::option::Option`, `::core::result::Result`, or `::statum::Branch`",
            ));
        }

        Ok(return_states)
    }
}

/// Represents the entire `impl` block of our `transition` macro
pub struct TransitionImpl {
    /// The concrete type being implemented (e.g. `Machine<Draft>`)
    pub target_type: Type,
    /// The machine type name extracted from `target_type` (e.g. `Machine`)
    pub machine_name: String,
    pub machine_span: Span,
    /// The source state extracted from `target_type` (e.g. `Draft`)
    pub source_state: String,
    pub source_state_span: Span,
    pub attrs: Vec<syn::Attribute>,
    /// All transition methods extracted from the `impl`
    pub functions: Vec<TransitionFn>,
}

pub fn parse_transition_impl(item_impl: &ItemImpl) -> Result<TransitionImpl, TokenStream> {
    let target_type = *item_impl.self_ty.clone();
    let Some((machine_name, machine_span, source_state, source_state_span)) =
        extract_impl_machine_and_state(&target_type)
    else {
        let message = LitStr::new(
            "Invalid #[transition] target type. Expected an impl target like `Machine<State>`.",
            target_type.span(),
        );
        return Err(quote::quote_spanned! { target_type.span() =>
            compile_error!(#message);
        });
    };

    let mut functions = Vec::new();
    for item in &item_impl.items {
        if let ImplItem::Fn(method) = item {
            functions.push(parse_transition_fn(method, &machine_name, &source_state)?);
        }
    }

    Ok(TransitionImpl {
        target_type,
        machine_name,
        machine_span,
        source_state,
        source_state_span,
        attrs: item_impl.attrs.clone(),
        functions,
    })
}

fn extract_impl_machine_and_state(target_type: &Type) -> Option<(String, Span, String, Span)> {
    let Type::Path(type_path) = target_type else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    extract_machine_state_from_segment(segment).map(|(_, state_name, state_span)| {
        (
            segment.ident.to_string(),
            segment.ident.span(),
            state_name,
            state_span,
        )
    })
}

pub fn parse_transition_fn(
    method: &ImplItemFn,
    machine_name: &str,
    source_state: &str,
) -> Result<TransitionFn, TokenStream> {
    let has_receiver = matches!(method.sig.inputs.first(), Some(FnArg::Receiver(_)));

    let return_type = match &method.sig.output {
        ReturnType::Type(_, ty) => Some(*ty.clone()),
        ReturnType::Default => None,
    };
    let return_type_span = match &method.sig.output {
        ReturnType::Type(_, ty) => Some(ty.span()),
        ReturnType::Default => None,
    };

    let generics = method
        .sig
        .generics
        .params
        .iter()
        .filter_map(|param| {
            if let syn::GenericParam::Type(type_param) = param {
                Some(type_param.ident.clone())
            } else {
                None
            }
        })
        .collect();

    let is_async = method.sig.asyncness.is_some();

    Ok(TransitionFn {
        name: method.sig.ident.clone(),
        attrs: method.attrs.clone(),
        presentation: parse_present_attrs(&method.attrs).map_err(|err| err.to_compile_error())?,
        has_receiver,
        return_type,
        return_type_span,
        machine_name: machine_name.to_owned(),
        source_state: source_state.to_owned(),
        generics,
        internals: method.block.clone(),
        is_async,
        vis: method.vis.to_owned(),
        span: method.span(),
    })
}

pub fn validate_transition_functions(
    tr_impl: &TransitionImpl,
    machine_info: &MachineInfo,
) -> Option<TokenStream> {
    if tr_impl.functions.is_empty() {
        let message = format!(
            "Error: #[transition] impl for `{}<{}>` must contain at least one method returning `{}` or the same machine path wrapped in a canonical `::core::option::Option<{}>`, `::core::result::Result<{}, E>`, or `::statum::Branch<{}, {}>`.",
            tr_impl.machine_name,
            tr_impl.source_state,
            machine_return_signature(&tr_impl.machine_name),
            machine_return_signature(&tr_impl.machine_name),
            machine_return_signature(&tr_impl.machine_name),
            machine_return_signature(&tr_impl.machine_name),
            machine_return_signature(&tr_impl.machine_name),
        );
        return Some(compile_error_at(tr_impl.target_type.span(), &message));
    }

    let state_enum_info = match machine_info.get_matching_state_enum() {
        Ok(enum_info) => enum_info,
        Err(err) => return Some(err),
    };

    if state_enum_info
        .get_variant_from_name(&tr_impl.source_state)
        .is_none()
    {
        return Some(invalid_transition_state_error(
            tr_impl.source_state_span,
            &tr_impl.machine_name,
            &tr_impl.source_state,
            &state_enum_info,
            "source",
        ));
    }

    for func in &tr_impl.functions {
        if !func.has_receiver {
            let message = format!(
                "Error: `#[transition]` method `{}<{}>::{}` must take `self` or `mut self` as its receiver.",
                tr_impl.machine_name,
                tr_impl.source_state,
                func.name,
            );
            return Some(compile_error_at(func.span, &message));
        }

        let return_state = match func.return_state(&tr_impl.target_type) {
            Ok(state) => state,
            Err(err) => return Some(err),
        };
        if state_enum_info.get_variant_from_name(&return_state).is_none() {
            return Some(invalid_transition_method_state_error(
                func,
                &tr_impl.machine_name,
                &return_state,
                &state_enum_info,
            ));
        }

        let return_states = match func.return_states(&tr_impl.target_type) {
            Ok(states) => states,
            Err(err) => return Some(err),
        };
        for return_state in return_states {
            if state_enum_info.get_variant_from_name(&return_state).is_none() {
                return Some(invalid_transition_method_state_error(
                    func,
                    &tr_impl.machine_name,
                    &return_state,
                    &state_enum_info,
                ));
            }
        }
    }

    None
}

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
    let Some(source_variant_info) = state_enum_info.get_variant_from_name(&tr_impl.source_state) else {
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
        let return_states = match function.return_states(target_type) {
            Ok(states) => states,
            Err(err) => return err,
        };

        for return_state in return_states {
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

                        impl #impl_generics statum::CanTransitionMap<#return_state_ident> for #target_type #where_clause {
                            type CurrentData = #source_data_ty;
                            type Output = #next_machine_ty;

                            fn transition_map<F>(self, f: F) -> Self::Output
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
        let return_states = match function.return_states(target_type) {
            Ok(states) => states,
            Err(err) => return err,
        };
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
    let transition_presentation_registrations = tr_impl.functions.iter().enumerate().filter_map(|(idx, function)| {
        let presentation = function.presentation.as_ref()?;
        let unique_suffix = transition_site_unique_suffix(tr_impl, function, idx);
        let token_ident = format_ident!("__STATUM_TRANSITION_TOKEN_{}", unique_suffix);
        let registration_ident =
            format_ident!("__STATUM_TRANSITION_PRESENTATION_{}", unique_suffix);
        let label = optional_lit_str_tokens(presentation.label.as_deref(), function.name.span());
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

fn strip_present_attrs_from_transition_impl(input: &ItemImpl) -> ItemImpl {
    let mut sanitized = input.clone();
    sanitized.attrs = strip_present_attrs(&sanitized.attrs);
    for item in &mut sanitized.items {
        if let ImplItem::Fn(method) = item {
            method.attrs = strip_present_attrs(&method.attrs);
        }
    }
    sanitized
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
            let metadata = syn::parse_str::<syn::Expr>(metadata_expr)
                .map_err(|err| err.to_compile_error())?;
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

pub fn missing_transition_machine_error(
    machine_name: &str,
    module_path: &str,
    span: Span,
) -> TokenStream {
    let current_line = current_source_info().map(|(_, line)| line).unwrap_or_default();
    let available = available_machine_candidates_in_module(module_path);
    let suggested_machine_name = available
        .first()
        .map(|candidate| candidate.name.as_str())
        .unwrap_or(machine_name);
    let available_line = if available.is_empty() {
        "No `#[machine]` items were found in this module.".to_string()
    } else {
        format!(
            "Available `#[machine]` items in this module: {}.",
            query::format_candidates(&available)
        )
    };
    let ordering_line = available
        .iter()
        .find(|candidate| candidate.name == machine_name && candidate.line_number > current_line)
        .map(|candidate| {
            format!(
                "Source scan found `#[machine]` item `{machine_name}` later in this module on line {}. If that item is active for this build, move it above this `#[transition]` impl because Statum resolves these relationships in expansion order.",
                candidate.line_number
            )
        })
        .map(|line| format!("{line}\n"))
        .unwrap_or_default();
    let elsewhere_line = same_named_machine_candidates_elsewhere(machine_name, module_path)
        .map(|candidates| {
            format!(
                "Same-named `#[machine]` items elsewhere in this file: {}.",
                query::format_candidates(&candidates)
            )
        })
        .unwrap_or_else(|| "No same-named `#[machine]` items were found in other modules of this file.".to_string());
    let missing_attr_line = plain_struct_line_in_module(module_path, machine_name).map(|line| {
        format!("A struct named `{machine_name}` exists on line {line}, but it is not annotated with `#[machine]`.")
    });
    let message = format!(
        "Error: no resolved `#[machine]` named `{machine_name}` was found in module `{module_path}`.\nStatum only resolves `#[machine]` items that have already expanded before this `#[transition]` impl. Include-generated transition fragments are only supported when the machine name is unique among the currently loaded machines in this crate.\n{ordering_line}{}\n{elsewhere_line}\n{available_line}\nHelp: apply `#[transition]` to an impl for the machine type generated by `#[machine]` in this module and declare that machine before the transition impl.\nCorrect shape: `#[transition] impl {suggested_machine_name}<CurrentState> {{ ... }}` where `{suggested_machine_name}` is declared with `#[machine]` in `{module_path}`.",
        missing_attr_line.unwrap_or_else(|| "No plain struct with that name was found in this module either.".to_string())
    );
    compile_error_at(span, &message)
}

pub fn ambiguous_transition_machine_error(
    machine_name: &str,
    module_path: &str,
    candidates: &[MachineInfo],
    span: Span,
) -> TokenStream {
    let candidate_line = crate::format_loaded_machine_candidates(candidates);
    let message = format!(
        "Error: resolved `#[machine]` named `{machine_name}` was ambiguous in module `{module_path}`.\nLoaded `#[machine]` candidates: {candidate_line}.\nHelp: keep one active `#[machine]` with that name in the module, or move conflicting machines into distinct modules."
    );
    compile_error_at(span, &message)
}

pub fn ambiguous_transition_machine_fallback_error(
    machine_name: &str,
    module_path: &str,
    candidates: &[MachineInfo],
    span: Span,
) -> TokenStream {
    let candidate_line = crate::format_loaded_machine_candidates(candidates);
    let message = format!(
        "Error: `#[transition]` impl for `{machine_name}` in module `{module_path}` could not use include-style fallback because the loaded machine name was ambiguous.\nLoaded `#[machine]` candidates: {candidate_line}.\nHelp: keep the machine name unique within the current crate when using include-generated transition fragments, or move the transition impl next to its machine definition."
    );
    compile_error_at(span, &message)
}

fn available_machine_candidates_in_module(module_path: &str) -> Vec<query::ItemCandidate> {
    let Some((file_path, _)) = current_source_info() else {
        return Vec::new();
    };
    query::candidates_in_module(&file_path, module_path, query::ItemKind::Struct, Some("machine"))
}

fn plain_struct_line_in_module(module_path: &str, struct_name: &str) -> Option<usize> {
    let (file_path, _) = current_source_info()?;
    query::plain_item_line_in_module(
        &file_path,
        module_path,
        query::ItemKind::Struct,
        struct_name,
        Some("machine"),
    )
}

fn invalid_transition_state_error(
    state_span: Span,
    machine_name: &str,
    state_name: &str,
    state_enum_info: &EnumInfo,
    role: &str,
) -> TokenStream {
    let valid_states = state_enum_info
        .variants
        .iter()
        .map(|variant| variant.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let target_type_display = format!("{machine_name}<{state_name}>");
    let state_enum_name = &state_enum_info.name;
    let message = format!(
        "Error: {role} state `{state_name}` in `#[transition]` target `{target_type_display}` is not a variant of `#[state]` enum `{state_enum_name}`.\nValid states for `{machine_name}` are: {valid_states}.\nHelp: change the impl target to `impl {machine_name}<ValidState>` using one of those variants."
    );
    compile_error_at(state_span, &message)
}

fn invalid_transition_method_state_error(
    func: &TransitionFn,
    machine_name: &str,
    return_state: &str,
    state_enum_info: &EnumInfo,
) -> TokenStream {
    let valid_states = state_enum_info
        .variants
        .iter()
        .map(|variant| variant.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let state_enum_name = &state_enum_info.name;
    let func_name = &func.name;
    let message = format!(
        "Error: transition method `{func_name}` returns state `{return_state}`, but `{return_state}` is not a variant of `#[state]` enum `{state_enum_name}`.\nValid next states for `{machine_name}` are: {valid_states}.\nHelp: return `{machine_name}<ValidState>` using one of those variants, or call `self.transition()` / `self.transition_with(...)`."
    );
    compile_error_at(func.return_type_span.unwrap_or(func.span), &message)
}

fn invalid_return_type_error(func: &TransitionFn, reason: &str) -> TokenStream {
    let func_name = &func.name;
    let return_type = func
        .return_type
        .as_ref()
        .map(|ty| ty.to_token_stream().to_string())
        .unwrap_or_else(|| "<none>".to_string());
    let machine_name = &func.machine_name;

    let message = format!(
        "Invalid transition return type for `{}<{}>::{func_name}`: {reason}.\n\n\
Expected:\n  fn {func_name}(self) -> {machine_name}<NextState>\n\n\
Actual:\n  {return_type}\n\n\
Help:\n  return `{machine_name}<NextState>` directly using the same machine path as the impl target, or wrap that same machine path in `::core::option::Option<...>`, `::core::result::Result<..., E>`, or `::statum::Branch<..., ...>` and build the next state with `self.transition()` or `self.transition_with(...)`.\n  Bare, aliased, or differently-qualified wrapper and machine paths are rejected because transition introspection only accepts exact syntactic return shapes."
        ,
        machine_name,
        func.source_state,
    );
    compile_error_at(func.return_type_span.unwrap_or(func.span), &message)
}

fn machine_return_signature(machine_name: &str) -> String {
    format!("{machine_name}<NextState>")
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
        tr_impl.machine_name,
        tr_impl.source_state,
        function.name,
        index,
        attrs,
        return_type,
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

fn compile_error_at(span: Span, message: &str) -> TokenStream {
    let message = LitStr::new(message, span);
    quote::quote_spanned! { span =>
        compile_error!(#message);
    }
}

/// Attempts to parse `ty` into the form:
///
///   - the same machine path as the impl target, with a different state marker
///   - `::core::option::Option<...>` or `::std::option::Option<...>`
///   - `::core::result::Result<..., E>` or `::std::result::Result<..., E>`
///   - `::statum::Branch<..., ...>` or `::statum_core::Branch<..., ...>`
///
/// On success, returns (`"Machine"`, `"SomeState"`).
pub fn parse_machine_and_state(ty: &Type, target_type: &Type) -> Option<(String, String)> {
    parse_primary_machine_and_state(ty, target_type)
}

/// Attempts to parse the primary visible next state from `ty`.
///
/// This preserves transition helper behavior by following the first generic
/// argument through supported wrappers until it reaches the same machine path
/// used by the impl target.
pub fn parse_primary_machine_and_state(ty: &Type, target_type: &Type) -> Option<(String, String)> {
    let mut current = ty;
    loop {
        match classify_primary_return_wrapper(current, target_type)? {
            PrimaryReturnWrapper::Machine(segment) => {
                return extract_machine_state_from_segment(segment)
                    .map(|(machine, state, _)| (machine, state));
            }
            PrimaryReturnWrapper::Option(inner)
            | PrimaryReturnWrapper::Result(inner)
            | PrimaryReturnWrapper::Branch(inner) => {
                current = inner;
            }
        }
    }
}

/// Collects every `Machine<State>` target mentioned in supported wrapper trees.
///
/// This is used for exact branch introspection and intentionally inspects both
/// sides of `Result<T, E>` while still ignoring arbitrary non-machine payloads.
pub fn collect_machine_and_states(ty: &Type, target_type: &Type) -> Vec<(String, String)> {
    let mut targets = Vec::new();
    collect_machine_targets(ty, target_type, &mut targets);
    targets
}

enum PrimaryReturnWrapper<'a> {
    Machine(&'a syn::PathSegment),
    Option(&'a Type),
    Result(&'a Type),
    Branch(&'a Type),
}

#[derive(Clone, Copy)]
enum SupportedWrapper {
    Option,
    Result,
    Branch,
}

fn classify_primary_return_wrapper<'a>(
    ty: &'a Type,
    target_type: &Type,
) -> Option<PrimaryReturnWrapper<'a>> {
    let type_path = type_path(ty)?;

    if let Some(segment) = machine_segment_matching_target(&type_path.path, target_type) {
        return Some(PrimaryReturnWrapper::Machine(segment));
    }

    let segment = type_path.path.segments.last()?;
    match supported_wrapper(&type_path.path)? {
        SupportedWrapper::Option => {
            extract_first_generic_type_ref(&segment.arguments).map(PrimaryReturnWrapper::Option)
        }
        SupportedWrapper::Result => {
            extract_first_generic_type_ref(&segment.arguments).map(PrimaryReturnWrapper::Result)
        }
        SupportedWrapper::Branch => {
            extract_first_generic_type_ref(&segment.arguments).map(PrimaryReturnWrapper::Branch)
        }
    }
}

fn collect_machine_targets(ty: &Type, target_type: &Type, targets: &mut Vec<(String, String)>) {
    let Some(type_path) = type_path(ty) else {
        return;
    };
    let Some(segment) = type_path.path.segments.last() else {
        return;
    };

    if machine_segment_matching_target(&type_path.path, target_type).is_some() {
        if let Some((machine, state, _)) = extract_machine_state_from_segment(segment) {
            push_unique_target(targets, machine, state);
        }
        return;
    }

    match supported_wrapper(&type_path.path) {
        Some(SupportedWrapper::Option) => {
            if let Some(inner) = extract_first_generic_type_ref(&segment.arguments) {
                collect_machine_targets(inner, target_type, targets);
            }
        }
        Some(SupportedWrapper::Result | SupportedWrapper::Branch) => {
            if let Some(types) = extract_generic_type_refs(&segment.arguments) {
                for inner in types {
                    collect_machine_targets(inner, target_type, targets);
                }
            }
        }
        None => {}
    }
}

fn push_unique_target(targets: &mut Vec<(String, String)>, machine: String, state: String) {
    if !targets.iter().any(|(existing_machine, existing_state)| {
        existing_machine == &machine && existing_state == &state
    }) {
        targets.push((machine, state));
    }
}

fn type_path(ty: &Type) -> Option<&TypePath> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    type_path.qself.is_none().then_some(type_path)
}

fn machine_segment_matching_target<'a>(
    candidate_path: &'a syn::Path,
    target_type: &Type,
) -> Option<&'a syn::PathSegment> {
    let target_path = &type_path(target_type)?.path;
    path_matches_target_machine(candidate_path, target_path)
        .then(|| candidate_path.segments.last())
        .flatten()
}

fn path_matches_target_machine(candidate: &syn::Path, target: &syn::Path) -> bool {
    if candidate.leading_colon.is_some() != target.leading_colon.is_some() {
        return false;
    }
    if candidate.segments.len() != target.segments.len() {
        return false;
    }

    let last_index = candidate.segments.len().saturating_sub(1);
    for (index, (candidate_segment, target_segment)) in
        candidate.segments.iter().zip(target.segments.iter()).enumerate()
    {
        if candidate_segment.ident != target_segment.ident {
            return false;
        }

        let arguments_match = if index == last_index {
            machine_generic_arguments_match(&candidate_segment.arguments, &target_segment.arguments)
        } else {
            path_arguments_equal(&candidate_segment.arguments, &target_segment.arguments)
        };

        if !arguments_match {
            return false;
        }
    }

    true
}

fn machine_generic_arguments_match(candidate: &PathArguments, target: &PathArguments) -> bool {
    let PathArguments::AngleBracketed(candidate_args) = candidate else {
        return false;
    };
    let PathArguments::AngleBracketed(target_args) = target else {
        return false;
    };
    if candidate_args.args.len() != target_args.args.len() || candidate_args.args.is_empty() {
        return false;
    }

    matches!(candidate_args.args.first(), Some(GenericArgument::Type(_)))
        && matches!(target_args.args.first(), Some(GenericArgument::Type(_)))
        && candidate_args
            .args
            .iter()
            .skip(1)
            .map(argument_tokens)
            .eq(target_args.args.iter().skip(1).map(argument_tokens))
}

fn path_arguments_equal(left: &PathArguments, right: &PathArguments) -> bool {
    argument_tokens(left) == argument_tokens(right)
}

fn argument_tokens<T: ToTokens>(tokens: &T) -> String {
    tokens.to_token_stream().to_string()
}

fn supported_wrapper(path: &syn::Path) -> Option<SupportedWrapper> {
    if matches_absolute_type_path(path, &["core", "option", "Option"])
        || matches_absolute_type_path(path, &["std", "option", "Option"])
    {
        return Some(SupportedWrapper::Option);
    }

    if matches_absolute_type_path(path, &["core", "result", "Result"])
        || matches_absolute_type_path(path, &["std", "result", "Result"])
    {
        return Some(SupportedWrapper::Result);
    }

    if matches_absolute_type_path(path, &["statum", "Branch"])
        || matches_absolute_type_path(path, &["statum_core", "Branch"])
    {
        return Some(SupportedWrapper::Branch);
    }

    None
}

fn matches_absolute_type_path(path: &syn::Path, expected: &[&str]) -> bool {
    path.leading_colon.is_some()
        && path.segments.len() == expected.len()
        && path
            .segments
            .iter()
            .zip(expected.iter())
            .enumerate()
            .all(|(index, (segment, expected_ident))| {
                segment.ident == *expected_ident
                    && (index + 1 == expected.len()
                        || matches!(segment.arguments, PathArguments::None))
            })
}

fn extract_machine_state_from_segment(segment: &syn::PathSegment) -> Option<(String, String, Span)> {
    extract_machine_generic(&segment.arguments, &segment.ident.to_string())
}

fn extract_machine_generic(args: &PathArguments, machine_name: &str) -> Option<(String, String, Span)> {
    let PathArguments::AngleBracketed(AngleBracketedGenericArguments {
        args: generic_args, ..
    }) = args
    else {
        return None;
    };
    let first_generic = generic_args.iter().find_map(|arg| match arg {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })?;
    let (state_name, state_span) = extract_state_marker(first_generic)?;
    Some((machine_name.to_string(), state_name, state_span))
}

fn extract_state_marker(ty: &Type) -> Option<(String, Span)> {
    let Type::Path(TypePath { qself: None, path }) = ty else {
        return None;
    };
    if path.leading_colon.is_some() || path.segments.len() != 1 {
        return None;
    }

    let state_segment = path.segments.last()?;
    if !matches!(state_segment.arguments, PathArguments::None) {
        return None;
    }

    Some((state_segment.ident.to_string(), state_segment.ident.span()))
}

fn same_named_machine_candidates_elsewhere(
    machine_name: &str,
    module_path: &str,
) -> Option<Vec<query::ItemCandidate>> {
    let (file_path, _) = current_source_info()?;
    let candidates = query::same_named_candidates_elsewhere(
        &file_path,
        module_path,
        query::ItemKind::Struct,
        machine_name,
        Some("machine"),
    );
    (!candidates.is_empty()).then_some(candidates)
}

fn extract_first_generic_type_ref(args: &PathArguments) -> Option<&Type> {
    extract_generic_type_refs(args)?.into_iter().next()
}

fn extract_generic_type_refs(args: &PathArguments) -> Option<Vec<&Type>> {
    let PathArguments::AngleBracketed(AngleBracketedGenericArguments {
        args: generic_args, ..
    }) = args
    else {
        return None;
    };

    let types = generic_args
        .iter()
        .filter_map(|arg| match arg {
            GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
        .collect::<Vec<_>>();
    if types.is_empty() {
        return None;
    }

    Some(types)
}

#[cfg(test)]
mod tests {
    use super::{
        collect_machine_and_states, extract_impl_machine_and_state, parse_machine_and_state,
        parse_primary_machine_and_state,
    };
    use syn::Type;

    fn parse_type(source: &str) -> Type {
        syn::parse_str(source).expect("valid type")
    }

    #[test]
    fn primary_parser_preserves_existing_result_behavior() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("::core::result::Result<Machine<Accepted>, Machine<Rejected>>");

        assert_eq!(
            parse_primary_machine_and_state(&ty, &target),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
        assert_eq!(
            parse_machine_and_state(&ty, &target),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
    }

    #[test]
    fn target_collector_reads_both_result_branches() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("::core::result::Result<Machine<Accepted>, Machine<Rejected>>");

        assert_eq!(
            collect_machine_and_states(&ty, &target),
            vec![
                ("Machine".to_owned(), "Accepted".to_owned()),
                ("Machine".to_owned(), "Rejected".to_owned()),
            ]
        );
    }

    #[test]
    fn primary_parser_reads_first_branch_target() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("::statum::Branch<Machine<Accepted>, Machine<Rejected>>");

        assert_eq!(
            parse_primary_machine_and_state(&ty, &target),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
        assert_eq!(
            parse_machine_and_state(&ty, &target),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
    }

    #[test]
    fn target_collector_reads_both_branch_targets() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("::statum::Branch<Machine<Accepted>, Machine<Rejected>>");

        assert_eq!(
            collect_machine_and_states(&ty, &target),
            vec![
                ("Machine".to_owned(), "Accepted".to_owned()),
                ("Machine".to_owned(), "Rejected".to_owned()),
            ]
        );
    }

    #[test]
    fn target_collector_reads_nested_wrappers() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type(
            "::core::option::Option<::core::result::Result<Machine<Accepted>, ::statum::Branch<Machine<Rejected>, Error>>>",
        );

        assert_eq!(
            collect_machine_and_states(&ty, &target),
            vec![
                ("Machine".to_owned(), "Accepted".to_owned()),
                ("Machine".to_owned(), "Rejected".to_owned()),
            ]
        );
    }

    #[test]
    fn target_collector_ignores_non_machine_payloads_and_dedups() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type(
            "::core::result::Result<::core::option::Option<Machine<Accepted>>, ::core::result::Result<Machine<Accepted>, Error>>",
        );

        assert_eq!(
            collect_machine_and_states(&ty, &target),
            vec![("Machine".to_owned(), "Accepted".to_owned())]
        );
    }

    #[test]
    fn parser_rejects_bare_wrappers() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("Result<Machine<Accepted>, Machine<Rejected>>");

        assert_eq!(parse_machine_and_state(&ty, &target), None);
        assert!(collect_machine_and_states(&ty, &target).is_empty());
    }

    #[test]
    fn parser_rejects_same_leaf_machine_in_other_module() {
        let target = parse_type("FlowMachine<Draft>");
        let ty = parse_type("other::FlowMachine<Done>");

        assert_eq!(parse_machine_and_state(&ty, &target), None);
        assert!(collect_machine_and_states(&ty, &target).is_empty());
    }

    #[test]
    fn parser_accepts_std_wrapper_paths() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type(
            "::std::option::Option<::std::result::Result<Machine<Accepted>, Error>>",
        );

        assert_eq!(
            parse_primary_machine_and_state(&ty, &target),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
        assert_eq!(
            collect_machine_and_states(&ty, &target),
            vec![("Machine".to_owned(), "Accepted".to_owned())]
        );
    }

    #[test]
    fn impl_target_rejects_qualified_state_paths() {
        let ty = parse_type("Machine<crate::Draft>");
        assert!(extract_impl_machine_and_state(&ty).is_none());
    }
}
