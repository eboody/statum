use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{FnArg, GenericArgument, Ident, PathArguments, ReturnType, Type};

use crate::VariantInfo;

use super::type_equivalence::types_equivalent;

pub(super) fn validator_state_name_from_ident(ident: &Ident) -> Option<String> {
    ident
        .to_string()
        .strip_prefix("is_")
        .map(std::borrow::ToOwned::to_owned)
}

pub(super) fn validate_validator_signature(
    func: &syn::ImplItemFn,
) -> Result<(), proc_macro2::TokenStream> {
    if func.sig.inputs.len() != 1 {
        let func_name = func.sig.ident.to_string();
        return Err(quote! {
            compile_error!(concat!("Error: ", #func_name, " must take exactly one argument: `&self`"));
        });
    }
    match &func.sig.inputs[0] {
        FnArg::Receiver(receiver) => {
            if receiver.reference.is_none() || receiver.mutability.is_some() {
                let func_name = func.sig.ident.to_string();
                return Err(quote! {
                    compile_error!(concat!("Error: ", #func_name, " must take `&self` as the first argument"));
                });
            }
        }
        FnArg::Typed(_) => {
            let func_name = func.sig.ident.to_string();
            return Err(quote! {
                compile_error!(concat!("Error: ", #func_name, " must take `&self` as the first argument"));
            });
        }
    }
    Ok(())
}

pub(super) fn expected_ok_type_for_variant(variant: &VariantInfo) -> Result<Type, TokenStream> {
    match &variant.data_type {
        Some(data_type) => syn::parse_str::<Type>(data_type).map_err(|err| err.to_compile_error()),
        None => Ok(syn::parse_quote!(())),
    }
}

pub(super) fn validate_validator_return_type(
    func: &syn::ImplItemFn,
    expected_ok_type: &Type,
) -> Result<(), TokenStream> {
    let ReturnType::Type(_, return_ty) = &func.sig.output else {
        let func_name = func.sig.ident.to_string();
        let expected_ok_display = expected_ok_type.to_token_stream().to_string();
        return Err(quote! {
            compile_error!(concat!(
                "Error: ", #func_name, " must return `Result<", #expected_ok_display, ", _>` (or an equivalent alias)"
            ));
        });
    };

    let actual_ok_ty = match extract_result_ok_type(return_ty) {
        Some(ty) => ty,
        None => {
            let func_name = func.sig.ident.to_string();
            let expected_ok_display = expected_ok_type.to_token_stream().to_string();
            return Err(quote! {
                compile_error!(concat!(
                    "Error: ", #func_name, " must return a `Result` type with payload `",
                    #expected_ok_display,
                    "`. Supported forms: `Result<T, E>`, `core::result::Result<T, E>`, `std::result::Result<T, E>`, and aliases like `statum::Result<T>`."
                ));
            });
        }
    };

    if !types_equivalent(&actual_ok_ty, expected_ok_type) {
        let func_name = func.sig.ident.to_string();
        let expected_ok_display = expected_ok_type.to_token_stream().to_string();
        let actual_return_type = return_ty.to_token_stream().to_string();
        let actual_ok_display = actual_ok_ty.to_token_stream().to_string();
        return Err(quote! {
            compile_error!(concat!(
                "Error: ", #func_name, " must return `Result<", #expected_ok_display, ", _>` (or an equivalent alias) but found `", #actual_return_type, "` with payload `", #actual_ok_display, "`"
            ));
        });
    }

    Ok(())
}

fn extract_result_ok_type(return_ty: &Type) -> Option<Type> {
    let Type::Path(type_path) = return_ty else {
        return None;
    };

    let last_segment = type_path.path.segments.last()?;
    if last_segment.ident != "Result" {
        return None;
    }

    let PathArguments::AngleBracketed(args) = &last_segment.arguments else {
        return None;
    };

    let type_args: Vec<Type> = args
        .args
        .iter()
        .filter_map(|arg| match arg {
            GenericArgument::Type(ty) => Some(ty.clone()),
            _ => None,
        })
        .collect();

    if type_args.is_empty() || type_args.len() > 2 || type_args.len() != args.args.len() {
        return None;
    }

    type_args.first().cloned()
}
