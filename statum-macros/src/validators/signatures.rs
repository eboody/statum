use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{FnArg, GenericArgument, Ident, Pat, PathArguments, ReturnType, Type};

pub(super) struct ValidatorDiagnosticContext<'a> {
    pub(super) persisted_type_display: &'a str,
    pub(super) machine_name: &'a str,
    pub(super) state_enum_name: &'a str,
    pub(super) variant_name: &'a str,
    pub(super) machine_fields: &'a [Ident],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ValidatorReturnKind {
    Plain,
    Diagnostic,
}

pub(super) struct AnalyzedValidatorReturn {
    pub(super) ok_type: Type,
    pub(super) return_kind: ValidatorReturnKind,
}

pub(super) fn validator_state_name_from_ident(ident: &Ident) -> Option<String> {
    ident
        .to_string()
        .strip_prefix("is_")
        .map(std::borrow::ToOwned::to_owned)
}

pub(super) fn validate_validator_signature(
    func: &syn::ImplItemFn,
    context: &ValidatorDiagnosticContext<'_>,
) -> Result<(), proc_macro2::TokenStream> {
    if func.sig.inputs.len() != 1 {
        let collision_line = explicit_param_collision_line(&func.sig.inputs, context.machine_fields);
        let expected_signature = expected_validator_signature(&func.sig.ident);
        let message = format!(
            "Error: validator `{}` for `impl {}` rebuilding `{}` state `{}::{}` must declare only `&self`.\nFound parameters: `({})`.\n{}\n{}\nCorrect shapes: {expected_signature}.",
            func.sig.ident,
            context.persisted_type_display,
            context.machine_name,
            context.state_enum_name,
            context.variant_name,
            func.sig
                .inputs
                .iter()
                .map(ToTokens::to_token_stream)
                .map(|tokens| tokens.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            collision_line.unwrap_or_else(|| {
                "Validator methods do not accept explicit machine-field parameters.".to_string()
            }),
            injected_machine_fields_line(context.machine_name, context.machine_fields),
        );
        let error = if let Some(extra_input) = func.sig.inputs.iter().nth(1) {
            syn::Error::new_spanned(extra_input, message)
        } else {
            syn::Error::new_spanned(&func.sig.inputs, message)
        };
        return Err(error.to_compile_error());
    }
    match &func.sig.inputs[0] {
        FnArg::Receiver(receiver) => {
            if receiver.reference.is_none() || receiver.mutability.is_some() {
                let receiver_display = receiver.to_token_stream().to_string();
                let expected_signature = expected_validator_signature(&func.sig.ident);
                let message = format!(
                    "Error: validator `{}` for `impl {}` rebuilding `{}` state `{}::{}` must take `&self`, not `{}`.\n{}\nCorrect shapes: {expected_signature}.",
                    func.sig.ident,
                    context.persisted_type_display,
                    context.machine_name,
                    context.state_enum_name,
                    context.variant_name,
                    receiver_display,
                    injected_machine_fields_line(context.machine_name, context.machine_fields),
                );
                return Err(syn::Error::new_spanned(receiver, message).to_compile_error());
            }
        }
        FnArg::Typed(_) => {
            let expected_signature = expected_validator_signature(&func.sig.ident);
            let message = format!(
                "Error: validator `{}` for `impl {}` rebuilding `{}` state `{}::{}` must take `&self` as its receiver.\n{}\nCorrect shapes: {expected_signature}.",
                func.sig.ident,
                context.persisted_type_display,
                context.machine_name,
                context.state_enum_name,
                context.variant_name,
                injected_machine_fields_line(context.machine_name, context.machine_fields),
            );
            return Err(syn::Error::new_spanned(&func.sig.inputs[0], message).to_compile_error());
        }
    }
    Ok(())
}

pub(super) fn analyze_validator_return_type(
    func: &syn::ImplItemFn,
    context: &ValidatorDiagnosticContext<'_>,
) -> Result<AnalyzedValidatorReturn, TokenStream> {
    let ReturnType::Type(_, return_ty) = &func.sig.output else {
        let message = format!(
            "Error: validator `{}` for `impl {}` rebuilding `{}` state `{}::{}` must return a supported validator result.\nSupported forms: `Result<T, E>`, `core::result::Result<T, E>`, `std::result::Result<T, E>`, aliases like `statum::Result<T>`, direct `Result<T, statum::Rejection>`, and `Validation<T>` / `statum::Validation<T>`.\nThe payload `T` must match the authoritative state data for `{}::{}`.",
            func.sig.ident,
            context.persisted_type_display,
            context.machine_name,
            context.state_enum_name,
            context.variant_name,
            context.state_enum_name,
            context.variant_name,
        );
        return Err(syn::Error::new_spanned(&func.sig.output, message).to_compile_error());
    };

    let (actual_ok_ty, return_kind) = match extract_supported_validator_ok_type(return_ty) {
        Some(info) => info,
        None => {
            let message = format!(
                "Error: validator `{}` for `impl {}` rebuilding `{}` state `{}::{}` must return a supported validator result.\nFound return type `{}`.\nSupported forms: `Result<T, E>`, `core::result::Result<T, E>`, `std::result::Result<T, E>`, aliases like `statum::Result<T>`, direct `Result<T, statum::Rejection>`, and `Validation<T>` / `statum::Validation<T>`.\nThe payload `T` must match the authoritative state data for `{}::{}`.",
                func.sig.ident,
                context.persisted_type_display,
                context.machine_name,
                context.state_enum_name,
                context.variant_name,
                return_ty.to_token_stream(),
                context.state_enum_name,
                context.variant_name,
            );
            return Err(syn::Error::new_spanned(return_ty, message).to_compile_error());
        }
    };

    Ok(AnalyzedValidatorReturn {
        ok_type: actual_ok_ty,
        return_kind,
    })
}

fn injected_machine_fields_line(machine_name: &str, machine_fields: &[Ident]) -> String {
    if machine_fields.is_empty() {
        format!(
            "Machine `{machine_name}` has no user-defined fields to inject, so validator methods should not take any extra parameters."
        )
    } else {
        let injected = machine_fields
            .iter()
            .map(|field| format!("`{field}`"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "Machine `{machine_name}` injects these fields by bare name inside validator bodies: {injected}. Remove explicit parameters and use those bindings directly."
        )
    }
}

fn expected_validator_signature(func_ident: &Ident) -> String {
    format!(
        "`fn {func_ident}(&self) -> Result<T, _>` or `fn {func_ident}(&self) -> Validation<T>`"
    )
}

fn explicit_param_collision_line(
    inputs: &syn::punctuated::Punctuated<FnArg, syn::Token![,]>,
    machine_fields: &[Ident],
) -> Option<String> {
    let collisions = inputs
        .iter()
        .skip(1)
        .filter_map(|arg| match arg {
            FnArg::Typed(arg) => match &*arg.pat {
                Pat::Ident(ident) if machine_fields.iter().any(|field| field == &ident.ident) => {
                    Some(ident.ident.to_string())
                }
                _ => None,
            },
            FnArg::Receiver(_) => None,
        })
        .collect::<Vec<_>>();

    if collisions.is_empty() {
        None
    } else {
        Some(format!(
            "Parameter name collision: {} {} with injected machine field {}.",
            collisions
                .iter()
                .map(|name| format!("`{name}`"))
                .collect::<Vec<_>>()
                .join(", "),
            if collisions.len() == 1 { "collides" } else { "collide" },
            if collisions.len() == 1 { "binding" } else { "bindings" }
        ))
    }
}

fn extract_supported_validator_ok_type(
    return_ty: &Type,
) -> Option<(Type, ValidatorReturnKind)> {
    let Type::Path(type_path) = return_ty else {
        return None;
    };

    let last_segment = type_path.path.segments.last()?;
    if last_segment.ident == "Validation" {
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

        if type_args.len() != 1 || type_args.len() != args.args.len() {
            return None;
        }

        return type_args
            .first()
            .cloned()
            .map(|ty| (ty, ValidatorReturnKind::Diagnostic));
    }

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

    let return_kind = if type_args.get(1).is_some_and(is_rejection_type) {
        ValidatorReturnKind::Diagnostic
    } else {
        ValidatorReturnKind::Plain
    };

    type_args.first().cloned().map(|ty| (ty, return_kind))
}

fn is_rejection_type(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };

    type_path
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Rejection")
}
