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

use crate::machine::transition_support_module_ident;
use crate::{EnumInfo, MachineInfo};

/// Stores all metadata for a single transition method in an `impl` block
#[allow(unused)]
pub struct TransitionFn {
    pub name: Ident,
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
    pub fn return_state(&self) -> Result<String, TokenStream> {
        let Some(return_type) = self.return_type.as_ref() else {
            return Err(invalid_return_type_error(self, "missing return type"));
        };
        let machine_ident = format_ident!("{}", self.machine_name);
        let Some((_, return_state)) = parse_machine_and_state(return_type, machine_ident) else {
            return Err(invalid_return_type_error(
                self,
                "expected return type like `Machine<NextState>` (optionally wrapped in `Option`/`Result`)",
            ));
        };

        Ok(return_state)
    }

    pub fn return_states(&self) -> Result<Vec<String>, TokenStream> {
        let Some(return_type) = self.return_type.as_ref() else {
            return Err(invalid_return_type_error(self, "missing return type"));
        };
        let machine_ident = format_ident!("{}", self.machine_name);
        let return_states = collect_machine_and_states(return_type, machine_ident)
            .into_iter()
            .map(|(_, state)| state)
            .collect::<Vec<_>>();
        if return_states.is_empty() {
            return Err(invalid_return_type_error(
                self,
                "expected return type like `Machine<NextState>` (optionally wrapped in `Option`/`Result`)",
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
            functions.push(parse_transition_fn(method, &machine_name, &source_state));
        }
    }

    Ok(TransitionImpl {
        target_type,
        machine_name,
        machine_span,
        source_state,
        source_state_span,
        functions,
    })
}

fn extract_impl_machine_and_state(target_type: &Type) -> Option<(String, Span, String, Span)> {
    let Type::Path(type_path) = target_type else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    extract_machine_generic(&segment.arguments, segment.ident.clone())
        .map(|(_, state_name, state_span)| {
            (
                segment.ident.to_string(),
                segment.ident.span(),
                state_name,
                state_span,
            )
        })
}

pub fn parse_transition_fn(method: &ImplItemFn, machine_name: &str, source_state: &str) -> TransitionFn {
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

    TransitionFn {
        name: method.sig.ident.clone(),
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
    }
}

pub fn validate_transition_functions(
    tr_impl: &TransitionImpl,
    machine_info: &MachineInfo,
) -> Option<TokenStream> {
    if tr_impl.functions.is_empty() {
        let message = format!(
            "Error: #[transition] impl for `{}<{}>` must contain at least one method returning `{}` or a supported wrapper like `Option<{}>` / `Result<{}, E>`.",
            tr_impl.machine_name,
            tr_impl.source_state,
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

        let return_state = match func.return_state() {
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

        let return_states = match func.return_states() {
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
    _module_path: &str,
) -> TokenStream {
    let target_type = &tr_impl.target_type;
    let machine_target_ident = format_ident!("{}", target_machine_info.name);
    let transition_support_module_ident = transition_support_module_ident(target_machine_info);
    let field_names = target_machine_info.field_names();
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

    let mut emitted_states = HashSet::new();
    let transition_impls = tr_impl.functions.iter().filter_map(|function| {
        let return_state = match function.return_state() {
            Ok(state) => state,
            Err(err) => return Some(err),
        };
        if !emitted_states.insert(return_state.clone()) {
            return None;
        }
        let return_state_ident = format_ident!("{}", return_state);
        let Some(variant_info) = state_enum_info.get_variant_from_name(&return_state) else {
            return Some(invalid_transition_method_state_error(
                function,
                &tr_impl.machine_name,
                &return_state,
                &state_enum_info,
            ));
        };

        Some(match variant_info.parse_data_type() {
            Ok(Some(data_ty)) => {
                quote! {
                    impl #transition_support_module_ident::TransitionWith<#data_ty> for #target_type {
                        type NextState = #return_state_ident;
                        fn transition_with(self, data: #data_ty) -> #machine_target_ident<Self::NextState> {
                            #machine_target_ident {
                                marker: core::marker::PhantomData,
                                state_data: data,
                                #(#field_names: self.#field_names,)*
                            }
                        }
                    }

                    impl statum::CanTransitionWith<#data_ty> for #target_type {
                        type NextState = #return_state_ident;
                        type Output = #machine_target_ident<#return_state_ident>;

                        fn transition_with_data(self, data: #data_ty) -> Self::Output {
                            <Self as #transition_support_module_ident::TransitionWith<#data_ty>>::transition_with(self, data)
                        }
                    }

                    impl statum::CanTransitionMap<#return_state_ident> for #target_type {
                        type CurrentData = #source_data_ty;
                        type Output = #machine_target_ident<#return_state_ident>;

                        fn transition_map<F>(self, f: F) -> Self::Output
                        where
                            F: FnOnce(Self::CurrentData) -> <#return_state_ident as statum::StateMarker>::Data,
                        {
                            let #machine_target_ident {
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
                    impl #transition_support_module_ident::TransitionTo<#return_state_ident> for #target_type {
                        fn transition(self) -> #machine_target_ident<#return_state_ident> {
                            #machine_target_ident {
                                marker: core::marker::PhantomData,
                                state_data: (),
                                #(#field_names: self.#field_names,)*
                            }
                        }
                    }

                    impl statum::CanTransitionTo<#return_state_ident> for #target_type {
                        type Output = #machine_target_ident<#return_state_ident>;

                        fn transition_to(self) -> Self::Output {
                            <Self as #transition_support_module_ident::TransitionTo<#return_state_ident>>::transition(self)
                        }
                    }
                }
            }
            Err(err) => err,
        })
    });

    quote! {
        #(#transition_impls)*
        #input
    }
}

pub fn missing_transition_machine_error(
    machine_name: &str,
    module_path: &str,
    span: Span,
) -> TokenStream {
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
        "Error: no `#[machine]` named `{machine_name}` was found in module `{module_path}`.\n{}\n{elsewhere_line}\n{available_line}\nHelp: apply `#[transition]` to an impl for the machine type generated by `#[machine]` in this module.\nCorrect shape: `#[transition] impl {suggested_machine_name}<CurrentState> {{ ... }}` where `{suggested_machine_name}` is declared with `#[machine]` in `{module_path}`.",
        missing_attr_line.unwrap_or_else(|| "No plain struct with that name was found in this module either.".to_string())
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
Help:\n  return `{machine_name}<NextState>` directly, or wrap it in `Option<...>` / `Result<..., E>` and build the next state with `self.transition()` or `self.transition_with(...)`."
        ,
        machine_name,
        func.source_state,
    );
    compile_error_at(func.return_type_span.unwrap_or(func.span), &message)
}

fn machine_return_signature(machine_name: &str) -> String {
    format!("{machine_name}<NextState>")
}

fn compile_error_at(span: Span, message: &str) -> TokenStream {
    let message = LitStr::new(message, span);
    quote::quote_spanned! { span =>
        compile_error!(#message);
    }
}

/// Attempts to parse `ty` into the form:
///
///   - `Machine<SomeState>`
///   - `Option<Machine<SomeState>>`
///   - `Result<Machine<SomeState>, E>`
///   - `some::error::Result<Machine<SomeState>, E>`
///
/// On success, returns ("Machine", "SomeState").
///
/// Walks through wrapper types (`Option`/`Result`) via their first generic argument.
pub fn parse_machine_and_state(ty: &Type, target_machine_ident: Ident) -> Option<(String, String)> {
    parse_primary_machine_and_state(ty, target_machine_ident)
}

/// Attempts to parse the primary visible next state from `ty`.
///
/// This preserves the existing transition helper behavior by following the first
/// generic argument through supported wrappers until it reaches `Machine<State>`.
pub fn parse_primary_machine_and_state(
    ty: &Type,
    target_machine_ident: Ident,
) -> Option<(String, String)> {
    let mut current = ty;
    loop {
        match classify_primary_return_wrapper(current, &target_machine_ident)? {
            PrimaryReturnWrapper::Machine(segment) => {
                return extract_machine_generic(&segment.arguments, target_machine_ident)
                    .map(|(machine, state, _)| (machine, state));
            }
            PrimaryReturnWrapper::Option(inner) | PrimaryReturnWrapper::Result(inner) => {
                current = inner;
            }
        }
    }
}

/// Collects every `Machine<State>` target mentioned in supported wrapper trees.
///
/// This is used for exact branch introspection and intentionally inspects both
/// sides of `Result<T, E>` while still ignoring arbitrary custom decision enums.
pub fn collect_machine_and_states(
    ty: &Type,
    target_machine_ident: Ident,
) -> Vec<(String, String)> {
    let mut targets = Vec::new();
    collect_machine_targets(ty, &target_machine_ident, &mut targets);
    targets
}

enum PrimaryReturnWrapper<'a> {
    Machine(&'a syn::PathSegment),
    Option(&'a Type),
    Result(&'a Type),
}

fn classify_primary_return_wrapper<'a>(
    ty: &'a Type,
    target_machine_ident: &Ident,
) -> Option<PrimaryReturnWrapper<'a>> {
    let Type::Path(TypePath { path, .. }) = ty else {
        return None;
    };
    let segment = path.segments.last()?;

    if &segment.ident == target_machine_ident {
        return Some(PrimaryReturnWrapper::Machine(segment));
    }

    if segment.ident == "Option" {
        return extract_first_generic_type_ref(&segment.arguments).map(PrimaryReturnWrapper::Option);
    }

    if segment.ident == "Result" {
        return extract_first_generic_type_ref(&segment.arguments).map(PrimaryReturnWrapper::Result);
    }

    None
}

fn collect_machine_targets(
    ty: &Type,
    target_machine_ident: &Ident,
    targets: &mut Vec<(String, String)>,
) {
    let Type::Path(TypePath { path, .. }) = ty else {
        return;
    };
    let Some(segment) = path.segments.last() else {
        return;
    };

    if &segment.ident == target_machine_ident {
        if let Some((machine, state, _)) =
            extract_machine_generic(&segment.arguments, target_machine_ident.clone())
        {
            push_unique_target(targets, machine, state);
        }
        return;
    }

    if segment.ident == "Option" {
        if let Some(inner) = extract_first_generic_type_ref(&segment.arguments) {
            collect_machine_targets(inner, target_machine_ident, targets);
        }
        return;
    }

    if segment.ident == "Result" {
        if let Some(types) = extract_generic_type_refs(&segment.arguments) {
            for inner in types {
                collect_machine_targets(inner, target_machine_ident, targets);
            }
        }
    }
}

fn push_unique_target(targets: &mut Vec<(String, String)>, machine: String, state: String) {
    if !targets
        .iter()
        .any(|(existing_machine, existing_state)| existing_machine == &machine && existing_state == &state)
    {
        targets.push((machine, state));
    }
}

fn extract_machine_generic(
    args: &PathArguments,
    target_machine_ident: Ident,
) -> Option<(String, String, Span)> {
    let PathArguments::AngleBracketed(AngleBracketedGenericArguments {
        args: generic_args, ..
    }) = args
    else {
        return None;
    };
    if generic_args.len() != 1 {
        return None;
    }
    let GenericArgument::Type(Type::Path(state_path)) = &generic_args[0] else {
        return None;
    };
    let state_seg = state_path.path.segments.last()?;
    Some((
        target_machine_ident.to_string(),
        state_seg.ident.to_string(),
        state_seg.ident.span(),
    ))
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
    use super::{collect_machine_and_states, parse_machine_and_state, parse_primary_machine_and_state};
    use quote::format_ident;
    use syn::Type;

    fn parse_type(source: &str) -> Type {
        syn::parse_str(source).expect("valid type")
    }

    #[test]
    fn primary_parser_preserves_existing_result_behavior() {
        let ty = parse_type("Result<Machine<Accepted>, Machine<Rejected>>");

        assert_eq!(
            parse_primary_machine_and_state(&ty, format_ident!("Machine")),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
        assert_eq!(
            parse_machine_and_state(&ty, format_ident!("Machine")),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
    }

    #[test]
    fn target_collector_reads_both_result_branches() {
        let ty = parse_type("Result<Machine<Accepted>, Machine<Rejected>>");

        assert_eq!(
            collect_machine_and_states(&ty, format_ident!("Machine")),
            vec![
                ("Machine".to_owned(), "Accepted".to_owned()),
                ("Machine".to_owned(), "Rejected".to_owned()),
            ]
        );
    }

    #[test]
    fn target_collector_reads_nested_wrappers() {
        let ty = parse_type("Option<core::result::Result<Machine<Accepted>, Machine<Rejected>>>");

        assert_eq!(
            collect_machine_and_states(&ty, format_ident!("Machine")),
            vec![
                ("Machine".to_owned(), "Accepted".to_owned()),
                ("Machine".to_owned(), "Rejected".to_owned()),
            ]
        );
    }

    #[test]
    fn target_collector_ignores_non_machine_payloads_and_dedups() {
        let ty = parse_type("Result<Option<Machine<Accepted>>, Result<Machine<Accepted>, Error>>");

        assert_eq!(
            collect_machine_and_states(&ty, format_ident!("Machine")),
            vec![("Machine".to_owned(), "Accepted".to_owned())]
        );
    }
}
