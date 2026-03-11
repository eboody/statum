use macro_registry::analysis::get_file_analysis;
use macro_registry::callsite::{current_source_info, module_path_for_line};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned, ToTokens};
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
    pub machine_name: String,
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
}

/// Represents the entire `impl` block of our `transition` macro
pub struct TransitionImpl {
    /// The concrete type being implemented (e.g. `Machine<Draft>`)
    pub target_type: Type,
    /// The machine type name extracted from `target_type` (e.g. `Machine`)
    pub machine_name: String,
    /// The source state extracted from `target_type` (e.g. `Draft`)
    pub source_state: String,
    /// All transition methods extracted from the `impl`
    pub functions: Vec<TransitionFn>,
}

pub fn parse_transition_impl(item_impl: &ItemImpl) -> Result<TransitionImpl, TokenStream> {
    let target_type = *item_impl.self_ty.clone();
    let Some((machine_name, source_state)) = extract_impl_machine_and_state(&target_type) else {
        let message = LitStr::new(
            "Invalid #[transition] target type. Expected an impl target like `Machine<State>`.",
            target_type.span(),
        );
        return Err(quote_spanned! { target_type.span() =>
            compile_error!(#message);
        });
    };

    let mut functions = Vec::new();
    for item in &item_impl.items {
        if let ImplItem::Fn(method) = item {
            functions.push(parse_transition_fn(method, &machine_name));
        }
    }

    Ok(TransitionImpl {
        target_type,
        machine_name,
        source_state,
        functions,
    })
}

fn extract_impl_machine_and_state(target_type: &Type) -> Option<(String, String)> {
    let Type::Path(type_path) = target_type else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    extract_machine_generic(&segment.arguments, segment.ident.clone())
}

pub fn parse_transition_fn(method: &ImplItemFn, machine_name: &str) -> TransitionFn {
    let has_receiver = matches!(method.sig.inputs.first(), Some(FnArg::Receiver(_)));

    let return_type = match &method.sig.output {
        ReturnType::Type(_, ty) => Some(*ty.clone()),
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
        machine_name: machine_name.to_owned(),
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
            &tr_impl.target_type,
            &tr_impl.machine_name,
            &tr_impl.source_state,
            &state_enum_info,
            "source",
        ));
    }

    for func in &tr_impl.functions {
        if !func.has_receiver {
            let func_name = &func.name;
            return Some(quote_spanned! { func.span =>
                compile_error!(concat!(
                    "Error: transition method `",
                    stringify!(#func_name),
                    "` must take `self` or `mut self` as its receiver."
                ));
            });
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

    let transition_impls = tr_impl.functions.iter().map(|function| {
        let return_state = match function.return_state() {
            Ok(state) => state,
            Err(err) => return err,
        };
        let return_state_ident = format_ident!("{}", return_state);
        let Some(variant_info) = state_enum_info.get_variant_from_name(&return_state) else {
            return invalid_transition_method_state_error(
                function,
                &tr_impl.machine_name,
                &return_state,
                &state_enum_info,
            );
        };

        match variant_info.parse_data_type() {
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
        }
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
    let available = available_machine_names_in_module(module_path);
    let available_line = if available.is_empty() {
        "No `#[machine]` items were found in this module.".to_string()
    } else {
        format!(
            "Available `#[machine]` items in this module: {}.",
            available.join(", ")
        )
    };
    let message = format!(
        "Error: no `#[machine]` named `{machine_name}` was found in module `{module_path}`.\n{available_line}\nFix: apply `#[transition]` to an impl for the machine type generated by `#[machine]` in this module."
    );
    compile_error_at(span, &message)
}

fn available_machine_names_in_module(module_path: &str) -> Vec<String> {
    let Some((file_path, _)) = current_source_info() else {
        return Vec::new();
    };
    let Some(analysis) = get_file_analysis(&file_path) else {
        return Vec::new();
    };

    let mut names = analysis
        .structs
        .iter()
        .filter(|entry| entry.attrs.iter().any(|attr| attr == "machine"))
        .filter(|entry| {
            module_path_for_line(&file_path, entry.line_number).as_deref() == Some(module_path)
        })
        .map(|entry| entry.item.ident.to_string())
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}

fn invalid_transition_state_error(
    target_type: &Type,
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
        "Error: {role} state `{state_name}` in `#[transition]` target `{target_type_display}` is not a variant of `#[state]` enum `{state_enum_name}`.\nValid states for `{machine_name}` are: {valid_states}."
    );
    compile_error_at(target_type.span(), &message)
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
        "Error: transition method `{func_name}` returns state `{return_state}`, but `{return_state}` is not a variant of `#[state]` enum `{state_enum_name}`.\nValid next states for `{machine_name}` are: {valid_states}."
    );
    compile_error_at(func.span, &message)
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
        "Invalid transition return type for `{func_name}`: {reason}.\n\n\
Expected:\n  fn {func_name}(self) -> {machine_name}<NextState>\n\n\
Actual:\n  {return_type}"
    );
    compile_error_at(func.span, &message)
}

fn machine_return_signature(machine_name: &str) -> String {
    format!("{machine_name}<NextState>")
}

fn compile_error_at(span: Span, message: &str) -> TokenStream {
    let message = LitStr::new(message, span);
    quote_spanned! { span =>
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
    let mut current = ty;
    loop {
        match classify_return_wrapper(current, &target_machine_ident)? {
            ReturnWrapper::Machine(segment) => {
                return extract_machine_generic(&segment.arguments, target_machine_ident);
            }
            ReturnWrapper::Option(inner) | ReturnWrapper::Result(inner) => {
                current = inner;
            }
        }
    }
}

enum ReturnWrapper<'a> {
    Machine(&'a syn::PathSegment),
    Option(&'a Type),
    Result(&'a Type),
}

fn classify_return_wrapper<'a>(
    ty: &'a Type,
    target_machine_ident: &Ident,
) -> Option<ReturnWrapper<'a>> {
    let Type::Path(TypePath { path, .. }) = ty else {
        return None;
    };
    let segment = path.segments.last()?;

    if &segment.ident == target_machine_ident {
        return Some(ReturnWrapper::Machine(segment));
    }

    if segment.ident == "Option" {
        return extract_first_generic_type_ref(&segment.arguments).map(ReturnWrapper::Option);
    }

    if segment.ident == "Result" {
        return extract_first_generic_type_ref(&segment.arguments).map(ReturnWrapper::Result);
    }

    None
}

fn extract_machine_generic(
    args: &PathArguments,
    target_machine_ident: Ident,
) -> Option<(String, String)> {
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
    ))
}

fn extract_first_generic_type_ref(args: &PathArguments) -> Option<&Type> {
    let PathArguments::AngleBracketed(AngleBracketedGenericArguments {
        args: generic_args, ..
    }) = args
    else {
        return None;
    };
    if generic_args.is_empty() {
        return None;
    }
    let GenericArgument::Type(ty) = &generic_args[0] else {
        return None;
    };
    Some(ty)
}
