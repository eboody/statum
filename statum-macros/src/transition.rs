use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use std::collections::HashMap;

use syn::{FnArg, ImplItem, ImplItemFn, ReturnType};

use syn::{Block, Ident, ItemImpl, Type};

use crate::{get_state_enum_variant, EnumInfo, MachineInfo, MachinePath, StateFilePath};

/// Stores all metadata for a single transition method in an `impl` block
#[derive(Debug)]
pub struct TransitionFn {
    pub name: Ident,
    pub args: Vec<TokenStream>,
    pub return_type: Option<Type>,
    pub generics: Vec<Ident>,
    pub internals: Block,
    pub is_async: bool,
    pub vis: syn::Visibility,
}

/// Represents the entire `impl` block of our `transition` macro
#[derive(Debug)]
pub struct TransitionImpl {
    /// The concrete type being implemented (e.g. `Machine<Draft>`)
    pub target_type: Type,
    /// All transition methods extracted from the `impl`
    pub functions: Vec<TransitionFn>,
}

pub fn parse_transition_impl(item_impl: &ItemImpl) -> TransitionImpl {
    // 1) Extract target type (e.g., `Machine<Draft>`)
    let target_type = *item_impl.self_ty.clone();

    // 2) Collect all transition methods
    let mut functions = Vec::new();
    for item in &item_impl.items {
        if let ImplItem::Fn(method) = item {
            functions.push(parse_transition_fn(method));
        }
    }

    TransitionImpl {
        target_type,
        functions,
    }
}

pub fn parse_transition_fn(method: &ImplItemFn) -> TransitionFn {
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

    // Collect generics
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

    TransitionFn {
        name: method.sig.ident.clone(),
        args,
        return_type,
        generics,
        internals: method.block.clone(),
        is_async,
        vis,
    }
}

/// Validate that the target type is a known Machine and the state is a valid variant.
/// Returns `Some(error_tokens)` if there is a validation error; otherwise `None`.
pub fn validate_machine_and_state(
    tr_impl: &TransitionImpl,
    file_path: &str,
    machine_map: &HashMap<MachinePath, MachineInfo>,
    state_enum_map: &HashMap<StateFilePath, EnumInfo>,
) -> Option<proc_macro2::TokenStream> {
    let target_type_str = tr_impl.target_type.to_token_stream().to_string();

    // e.g. split at '<' to separate "Machine" from "Draft>"
    let mut type_parts = target_type_str.split('<');
    let machine_name = type_parts.next().unwrap_or("").trim().to_string();
    let state_name = type_parts
        .next()
        .map(|s| s.trim_end_matches('>').trim().to_string());

    // 1) Validate machine
    let machine_info = machine_map.get(&file_path.into());
    if machine_info.is_none() {
        let machine_name_clone = machine_name.clone();
        return Some(quote! {
            compile_error!(
                concat!(
                    "Invalid machine: ",
                    #machine_name_clone,
                    " is not present in this file. Did you forget to add a #[machine] attribute?"
                )
            )
        });
    }

    // 2) Validate state variant
    let found_enum_and_variant = state_enum_map.iter().any(|(state_enum_file_path, info)| {
        // We only match the same file path
        state_enum_file_path == &file_path.to_owned().into()
            && state_name
                .as_ref()
                .is_some_and(|state| info.variants.iter().any(|variant| &variant.name == state))
    });

    let associated_state_enum = state_enum_map
        .iter()
        .find(|(state_enum_file_path, _)| state_enum_file_path.as_ref() == file_path)
        .expect("Expected a state enum for this file");
    let associated_state_enum_name = &associated_state_enum.1.name;
    if !found_enum_and_variant {
        return Some(quote! {
            compile_error!(
                concat!(
                    "Invalid state variant: ",
                    #state_name,
                    " is not a valid variant of a registered #[state] enum: ",
                    #associated_state_enum_name
                )
            )
        });
    }

    None
}

/// Validate all transition function signatures:
///  - must have exactly one argument
///  - that argument must be "self"
pub fn validate_transition_functions(
    functions: &[TransitionFn],
) -> Option<proc_macro2::TokenStream> {
    if functions.is_empty() {
        return Some(quote! {
            compile_error!("#[transition] impl blocks must contain at least one method returning Machine<SomeState>.");
        });
    }

    for func in functions {
        if func.args[0].to_string() != "self" && func.args[0].to_string() != "mut self" {
            let func_name = &func.name;
            return Some(quote! {
                compile_error!(
                    concat!(
                        "Invalid function signature: ",
                        stringify!(#func_name),
                        " transition functions must be a method, that is it must take 'self' or 'mut self' as it's first argument."
                    )
                )
            });
        }
    }
    None
}

pub fn generate_transition_impl(
    tr_impl: &TransitionImpl,
    target_machine_info: &MachineInfo,
    file_path: &str,
) -> proc_macro2::TokenStream {
    let target_type = &tr_impl.target_type;
    let machine_target_ident = format_ident!("{}", target_machine_info.name);

    let (_, target_state) = parse_machine_and_state(target_type, machine_target_ident.clone())
        .expect("Expected a state name");

    let target_state_variant = get_state_enum_variant(&file_path.into(), &target_state)
        .expect("Expected a valid state variant. This should have been validated earlier.");

    // Then generate code for each user-defined function
    let user_fns = tr_impl.functions.iter().map(|function| {
        let name = &function.name;
        let args = function.args.clone();
        let generics = function.generics.iter().map(|gen| format_ident!("{}", gen));
        let return_type = function.return_type.as_ref().map(|ty| quote!(-> #ty));

        let block = &function.internals; // syn::Block
        // If the block uses get_data_mut, then check for Clone on machine and state

        // Prepare an empty token stream to potentially hold an error
        let mut extra_tokens = quote! {};
        if contains_get_data_mut(block) {
            if !target_machine_info
                .derives
                .iter()
                .any(|d| d.trim() == "Clone")
            {
                extra_tokens = quote! {
                    compile_error!("Using get_data_mut requires that the machine struct derive Clone. Please add #[derive(Clone)] to your machine struct.");
                };
            } else {
                let state_enum = target_machine_info.get_matching_state_enum();
                if !state_enum.derives.iter().any(|d| d.trim() == "Clone") {
                    extra_tokens = quote! {
                        compile_error!("Using get_data_mut requires that the state enum derive Clone. Please add #[derive(Clone)] to your #[state] enum.");
                    };
                }
            }
        }
        let mut transition_impl = quote! {};

        if let Some(ref return_type) = function.return_type {
            let (_return_machine_name, return_state_name) =
                parse_machine_and_state(return_type, machine_target_ident.clone()).expect("Expected a state name");

            let return_state_info = get_state_enum_variant(&file_path.into(), &return_state_name)
                .expect("Expected a valid state variant. This should have been validated earlier.");

            let fields = target_machine_info.fields_to_token_stream();

            let return_state = format_ident!("{}", return_state_name);

            transition_impl = if let Some(data_type) = &return_state_info.data_type {
                let data_type = format_ident!("{}", data_type);
                quote! {
                    pub fn transition(self, data: #data_type) -> #machine_target_ident<#return_state>
                    {
                        #machine_target_ident {
                            #fields
                            marker: core::marker::PhantomData,
                            state_data: data,
                        }
                    }
                }
            } else {
                quote! {
                    pub fn transition(self) -> #machine_target_ident<#return_state>
                    {
                        #machine_target_ident {
                            #fields
                            marker: core::marker::PhantomData,
                            state_data: (),
                        }
                    }
                }
            }
        }

        let mut get_data_impl = quote! {};

        if let Some(data_type) = &target_state_variant.data_type {
            let data_type = format_ident!("{}", data_type);

            get_data_impl = quote! {
                pub fn get_data(&self) -> &#data_type {
                    &self.state_data
                }

                pub fn get_data_mut(&mut self) -> &mut #data_type {
                    &mut self.state_data
                }
            }
        }

        let async_token = if function.is_async { quote! { async } } else { quote! {} };
        let vis_token = &function.vis;

        quote! {
            #transition_impl
            #vis_token #async_token fn #name<#(#generics),*>(#(#args),*) #return_type
            #block

            #extra_tokens
            #get_data_impl
        }
    });

    quote! {
        impl #target_type {
            #(#user_fns)*
        }
    }
}
use syn::visit::Visit;

struct GetDataMutVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for GetDataMutVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "get_data_mut" {
            self.found = true;
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

fn contains_get_data_mut(block: &syn::Block) -> bool {
    let mut visitor = GetDataMutVisitor { found: false };
    syn::visit::visit_block(&mut visitor, block);
    visitor.found
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

    let ident_str = last_segment.ident.to_string();

    // 1) If it's `Machine`, parse the single generic as `SomeState`.
    if target_machine_ident == ident_str {
        return extract_machine_generic(&last_segment.arguments, target_machine_ident);
    }

    // 2) If it's `Option`, parse the first generic argument -> presumably `Machine<SomeState>`.
    if ident_str == "Option" {
        if let Some(inner_ty) = extract_first_generic_type(&last_segment.arguments) {
            return parse_machine_and_state(&inner_ty, target_machine_ident);
        }
        return None;
    }

    // 3) If it's `Result`, parse the first generic argument -> presumably `Machine<SomeState>`.
    if ident_str == "Result" {
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
