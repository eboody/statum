use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::spanned::Spanned;

use syn::{FnArg, ImplItem, ImplItemFn, ReturnType};

use syn::Block;
use syn::{Ident, ItemImpl, Type};

use crate::MachineInfo;

/// Stores all metadata for a single transition method in an `impl` block
#[allow(unused)]
pub struct TransitionFn {
    pub name: Ident,
    pub args: Vec<TokenStream>,
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
    /// All transition methods extracted from the `impl`
    pub functions: Vec<TransitionFn>,
}

pub fn parse_transition_impl(item_impl: &ItemImpl) -> TransitionImpl {
    // 1) Extract target type (e.g., `Machine<Draft>`)
    let target_type = *item_impl.self_ty.clone();

    let machine_name = target_type
        .to_token_stream()
        .to_string()
        .split('<')
        .next()
        .unwrap()
        .trim()
        .to_string();

    // 2) Collect all transition methods
    let mut functions = Vec::new();
    for item in &item_impl.items {
        if let ImplItem::Fn(method) = item {
            functions.push(parse_transition_fn(method, &machine_name));
        }
    }

    TransitionImpl {
        target_type,
        functions,
    }
}

pub fn parse_transition_fn(method: &ImplItemFn, machine_name: &str) -> TransitionFn {
    // Collect argument names/types with receiver details.

    let args: Vec<proc_macro2::TokenStream> = method
        .sig
        .inputs
        .iter()
        .map(|arg| match arg {
            FnArg::Receiver(receiver) => {
                let mutability = receiver.mutability;
                quote! { #mutability self }
            }
            FnArg::Typed(pat_type) => {
                let arg_ty = pat_type.ty.to_token_stream();
                quote! { #arg_ty }
            }
        })
        .collect();

    // Collect return type if any
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

    let vis = method.vis.to_owned();
    let span = method.span();

    TransitionFn {
        name: method.sig.ident.clone(),
        args,
        return_type,
        machine_name: machine_name.to_owned(),
        generics,
        internals: method.block.clone(),
        is_async,
        vis,
        span,
    }
}

/// Validate that the target type is a known Machine and the state is a valid variant.
/// Returns `Some(error_tokens)` if there is a validation error; otherwise `None`.
// Validation of machine/state names is handled by the type system now.

/// Validate all transition function signatures:
///  - must have exactly one argument: `self` or `mut self`
///  - if the return type is Machine<T> where T is a state variant that carries data,
///    then the function must call `.transition_with(..)` instead of `.transition()`
pub fn validate_transition_functions(
    functions: &[TransitionFn],
    _machine_info: &MachineInfo,
) -> Option<proc_macro2::TokenStream> {
    if functions.is_empty() {
        return Some(quote! {
            compile_error!("#[transition] impl blocks must contain at least one method returning a valid machine.");
        });
    }

    for func in functions {
        if func.args.is_empty() {
            let func_name = &func.name;
            return Some(quote_spanned! { func.span =>
                compile_error!(
                    concat!(
                        "Invalid function signature: ",
                        stringify!(#func_name),
                        " must take `self` or `mut self` as the first argument."
                    )
                );
            });
        }
        // Ensure the first argument is either 'self' or 'mut self'
        if func.args[0].to_string() != "self" && func.args[0].to_string() != "mut self" {
            let func_name = &func.name;
            return Some(quote_spanned! { func.span =>
                compile_error!(
                    concat!(
                        "Invalid function signature: ",
                        stringify!(#func_name),
                        " transition functions must be a method, that is, it must take 'self' or 'mut self' as its first argument."
                    )
                );
            });
        }

        if let Err(err) = func.return_state() {
            return Some(err);
        }
    }
    None
}

pub fn generate_transition_impl(
    input: &ItemImpl,
    tr_impl: &TransitionImpl,
    target_machine_info: &MachineInfo,
    _module_path: &str,
) -> proc_macro2::TokenStream {
    let target_type = &tr_impl.target_type; // e.g., `OrderMachine<Cart>`
    let machine_target_ident = format_ident!("{}", target_machine_info.name);
    let field_names = target_machine_info.field_names();
    let state_enum_info = match target_machine_info.get_matching_state_enum() {
        Ok(enum_info) => enum_info,
        Err(err) => return err,
    };
    let state_enum_name = state_enum_info.name.clone();

    // Iterate over transition functions
    let transition_impls = tr_impl.functions.iter().map(|function| {
        let return_state = match function.return_state() {
            Ok(state) => state,
            Err(err) => return err,
        };
        let return_state_ident = format_ident!("{}", return_state);
        let Some(variant_info) = state_enum_info.get_variant_from_name(&return_state) else {
            return quote_spanned! { function.span =>
                compile_error!(concat!(
                    "Invalid state variant: ",
                    #return_state,
                    " is not a valid variant of a registered #[state] enum: ",
                    #state_enum_name
                ));
            };
        };

        match &variant_info.data_type {
            Some(data_type) => {
                let data_ty = syn::parse_str::<Type>(data_type).unwrap();
                quote! {
                    impl TransitionWith<#data_ty> for #target_type {
                        type NextState = #return_state_ident;
                        fn transition_with(self, data: #data_ty) -> #machine_target_ident<Self::NextState> {
                            #machine_target_ident {
                                marker: core::marker::PhantomData,
                                state_data: data,
                                #(#field_names: self.#field_names,)*
                            }
                        }
                    }
                }
            }
            None => {
                quote! {
                    impl TransitionTo<#return_state_ident> for #target_type {
                        fn transition(self) -> #machine_target_ident<#return_state_ident> {
                            #machine_target_ident {
                                marker: core::marker::PhantomData,
                                state_data: (),
                                #(#field_names: self.#field_names,)*
                            }
                        }
                    }
                }
            }
        }
    });

    quote! {
        #(#transition_impls)*
        #input // Append the original impl block
    }
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
    let message = syn::LitStr::new(&message, func.span);

    quote_spanned! { func.span =>
        compile_error!(#message);
    }
}

use syn::{AngleBracketedGenericArguments, GenericArgument, PathArguments, TypePath};

/// Attempts to parse `ty` into the form:
///
///   - `Machine<SomeState>`
///   - `Option<Machine<SomeState>>`
///   - `Result<Machine<SomeState>, E>`
///   - `some::error::Result<Machine<SomeState>, E>`
///
/// On success, returns ("Machine", "SomeState").
///
/// Recurses on the *first* generic argument if the last segment is `Result` or `Option`.
pub fn parse_machine_and_state(ty: &Type, target_machine_ident: Ident) -> Option<(String, String)> {
    // We only handle `Type::Path` (e.g. `Machine<...>`, `Option<...>`, `Result<...>`, etc.)
    let Type::Path(TypePath { path, .. }) = ty else {
        return None;
    };

    // Extract the LAST segment in the path, even if it has multiple segments (e.g. `some::error::Result`)
    let last_segment = path.segments.last()?;

    let return_machine_ident = last_segment.ident.clone();

    // 1) If it's `Machine`, parse the single generic as `SomeState`.
    if target_machine_ident == return_machine_ident {
        return extract_machine_generic(&last_segment.arguments, target_machine_ident);
    }

    // 2) If it's `Option`, parse the first generic argument -> presumably `Machine<SomeState>`.
    if return_machine_ident == "Option" {
        if let Some(inner_ty) = extract_first_generic_type(&last_segment.arguments) {
            return parse_machine_and_state(&inner_ty, target_machine_ident);
        }
        return None;
    }

    // 3) If it's `Result`, parse the first generic argument -> presumably `Machine<SomeState>`.
    if return_machine_ident == "Result" {
        if let Some(inner_ty) = extract_first_generic_type(&last_segment.arguments) {
            return parse_machine_and_state(&inner_ty, target_machine_ident);
        }
        return None;
    }

    // 4) If the last segment isn't recognized, no match.
    None
}

/// Extracts `Machine<SomeState>` from the angle brackets of the given path arguments.
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
    // We expect exactly one generic argument: `Machine<OneThing>`
    if generic_args.len() != 1 {
        return None;
    }
    // That one argument must be a `TypePath`, e.g. `InProgress`
    let GenericArgument::Type(Type::Path(state_path)) = &generic_args[0] else {
        return None;
    };
    let state_seg = state_path.path.segments.last()?;
    // Return ("Machine", "InProgress") for example
    Some((
        target_machine_ident.to_string(),
        state_seg.ident.to_string(),
    ))
}

/// Extracts the *first* generic type from an angle-bracketed path, e.g:
///   `Option< T >` -> returns `Some(T)`
///   `Result< T, E >` -> returns `Some(T)`
///
/// Otherwise returns `None`.
fn extract_first_generic_type(args: &PathArguments) -> Option<Type> {
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
    Some(ty.clone())
}
