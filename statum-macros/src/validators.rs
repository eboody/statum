use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote};
use std::collections::HashMap;
use syn::{
    FnArg, GenericArgument, Ident, ItemImpl, PathArguments, ReturnType, Type, parse_macro_input,
};

use crate::{
    get_state_enum, EnumInfo, MachineInfo, MachinePath, StateModulePath, VariantInfo,
    ensure_machine_loaded_by_name, ensure_state_enum_loaded_by_name, to_snake_case,
};

fn has_validators(item: &ItemImpl, state_variants: &[VariantInfo]) -> proc_macro2::TokenStream {
    if item.items.is_empty() {
        return quote! {};
    }

    let mut missing = Vec::new();
    for variant in state_variants {
        let variant_name = to_snake_case(&variant.name);
        let has_validator = item
            .items
            .iter()
            .filter_map(|item| {
                if let syn::ImplItem::Fn(func) = item {
                    validator_state_name_from_ident(&func.sig.ident)
                } else {
                    None
                }
            })
            .any(|state_name| state_name == variant_name);

        if !has_validator {
            missing.push(variant_name);
        }
    }

    if !missing.is_empty() {
        let missing_list = missing
            .iter()
            .map(|name| format!("is_{name}"))
            .collect::<Vec<_>>()
            .join(", ");
        return quote! {
            compile_error!(concat!(
                "Error: missing validator methods: ",
                #missing_list,
                ".\n",
                "Fix: add one validator per state variant (snake_case), e.g. `fn is_draft(&self) -> Result<()>`."
            ));
        };
    }

    quote! {}
}

pub fn parse_validators(attr: TokenStream, item: TokenStream, module_path: &str) -> TokenStream {
    let machine_ident = parse_macro_input!(attr as Ident);
    let item_impl = parse_macro_input!(item as ItemImpl);
    let struct_ident = &item_impl.self_ty;

    let machine_metadata = match resolve_machine_metadata(module_path, &machine_ident) {
        Ok(metadata) => metadata,
        Err(err) => return err.into(),
    };

    let modified_methods = match inject_machine_fields(&item_impl.items, &machine_metadata) {
        Ok(methods) => methods,
        Err(err) => return err.into(),
    };

    let state_enum_info = match resolve_state_enum_info(module_path, &machine_metadata) {
        Ok(info) => info,
        Err(err) => return err.into(),
    };

    let has_validators = has_validators(&item_impl, &state_enum_info.variants);

    let field_names = machine_metadata.field_names();
    let superstate_ident = format_ident!("{}SuperState", machine_ident);

    let (validator_checks, has_async) = match collect_validator_checks(
        &item_impl,
        &machine_ident,
        &superstate_ident,
        &field_names,
        &state_enum_info.variants,
    ) {
        Ok(result) => result,
        Err(err) => return err.into(),
    };

    let fields_with_types = match machine_metadata.fields_with_types() {
        Ok(fields) => fields,
        Err(err) => return err.to_compile_error().into(),
    };

    if item_impl.items.is_empty() {
        return quote! {
            compile_error!("Error: No validator functions found in impl block. Add at least one `is_*` method.");
        }
        .into();
    }

    let machine_vis: syn::Visibility = match syn::parse_str(&machine_metadata.vis) {
        Ok(vis) => vis,
        Err(_) => syn::parse_quote!( /* default or nothing */ ),
    };

    let async_token = if has_async {
        quote! { async }
    } else {
        quote! {}
    };

    let batch_builder_impl = batch_builder_implementation(
        &machine_ident,
        struct_ident,
        &superstate_ident,
        &machine_metadata,
        async_token.clone(),
        machine_vis.clone(),
    );

    // **Fill in `new()` with the validation logic**
    let machine_builder_impl = quote! {
        #[statum::bon::bon(crate = ::statum::bon)]
        impl #struct_ident {
            #[builder(start_fn = machine_builder)]
            #machine_vis #async_token fn new(&self #(, #fields_with_types)*) -> core::result::Result<#superstate_ident, statum::Error> {
                #(#validator_checks)*

                Err(statum::Error::InvalidState)
            }
            #[builder(start_fn = into_machine, finish_fn = build)]
            #machine_vis #async_token fn __statum_into_machine(&self #(, #fields_with_types)*) -> core::result::Result<#superstate_ident, statum::Error> {
                #(#validator_checks)*

                Err(statum::Error::InvalidState)
            }
            #(#modified_methods)*
        }

        #batch_builder_impl
    };

    // Merge original item with generated code
    let expanded = quote! {
        #has_validators
        #machine_builder_impl
    };

    expanded.into()
}

fn resolve_machine_metadata(
    module_path: &str,
    machine_ident: &Ident,
) -> Result<MachineInfo, proc_macro2::TokenStream> {
    let module_path_key: MachinePath = module_path.into();
    let machine_name = machine_ident.to_string();
    ensure_machine_loaded_by_name(&module_path_key, &machine_name).ok_or_else(|| {
        quote! {
            compile_error!("Error: No matching `#[machine]` found in scope. Ensure `#[validators(Machine)]` references a machine in the same module.");
        }
    })
}

fn resolve_state_enum_info(
    module_path: &str,
    machine_metadata: &MachineInfo,
) -> Result<EnumInfo, proc_macro2::TokenStream> {
    let state_path_key: StateModulePath = module_path.into();
    let expected_state_name = machine_metadata.expected_state_name();
    let _ = if let Some(expected_name) = expected_state_name.as_ref() {
        ensure_state_enum_loaded_by_name(&state_path_key, expected_name)
    } else {
        None
    };

    let state_enum_info = match expected_state_name {
        Some(expected_name) => ensure_state_enum_loaded_by_name(&state_path_key, &expected_name),
        None => get_state_enum(&state_path_key),
    };
    state_enum_info.ok_or_else(|| {
        quote! {
            compile_error!(
                "Error: No matching #[state] enum found in this module. \
Ensure the enum is in the same module as the machine and validators, and that the machine's first generic parameter matches the #[state] enum name."
            );
        }
    })
}

fn collect_validator_checks(
    item_impl: &ItemImpl,
    machine_ident: &Ident,
    superstate_ident: &Ident,
    field_names: &[Ident],
    variants: &[VariantInfo],
) -> Result<(Vec<proc_macro2::TokenStream>, bool), proc_macro2::TokenStream> {
    let mut checks = Vec::new();
    let mut has_async = false;
    let variant_by_name = variant_lookup(variants);

    for item in &item_impl.items {
        let syn::ImplItem::Fn(func) = item else {
            continue;
        };

        let Some(state_name) = validator_state_name_from_ident(&func.sig.ident) else {
            continue;
        };
        validate_validator_signature(func)?;

        let Some(state_variant) = variant_by_name.get(&state_name).cloned() else {
            continue;
        };
        validate_validator_return_type(func, &state_variant)?;

        if func.sig.asyncness.is_some() {
            has_async = true;
        }
        checks.push(generate_validator_check(
            machine_ident,
            superstate_ident,
            field_names,
            &state_variant,
            func.sig.asyncness.is_some(),
        ));
    }

    Ok((checks, has_async))
}

fn variant_lookup(variants: &[VariantInfo]) -> HashMap<String, VariantInfo> {
    let mut variant_by_name = HashMap::with_capacity(variants.len() * 2);
    for variant in variants {
        variant_by_name.insert(variant.name.clone(), variant.clone());
        variant_by_name.insert(to_snake_case(&variant.name), variant.clone());
    }
    variant_by_name
}

fn validator_state_name_from_ident(ident: &Ident) -> Option<String> {
    ident
        .to_string()
        .strip_prefix("is_")
        .map(std::borrow::ToOwned::to_owned)
}

fn validate_validator_signature(func: &syn::ImplItemFn) -> Result<(), proc_macro2::TokenStream> {
    let func_name = func.sig.ident.to_string();
    if func.sig.inputs.len() != 1 {
        return Err(quote! {
            compile_error!(concat!("Error: ", #func_name, " must take exactly one argument: `&self`"));
        });
    }
    match &func.sig.inputs[0] {
        FnArg::Receiver(receiver) => {
            if receiver.reference.is_none() || receiver.mutability.is_some() {
                return Err(quote! {
                    compile_error!(concat!("Error: ", #func_name, " must take `&self` as the first argument"));
                });
            }
        }
        FnArg::Typed(_) => {
            return Err(quote! {
                compile_error!(concat!("Error: ", #func_name, " must take `&self` as the first argument"));
            });
        }
    }
    Ok(())
}

fn expected_ok_type_for_variant(variant: &VariantInfo) -> Result<Type, proc_macro2::TokenStream> {
    match &variant.data_type {
        Some(data_type) => {
            syn::parse_str::<Type>(data_type).map_err(|err| err.to_compile_error())
        }
        None => Ok(syn::parse_quote!(())),
    }
}

fn validate_validator_return_type(
    func: &syn::ImplItemFn,
    state_variant: &VariantInfo,
) -> Result<(), proc_macro2::TokenStream> {
    let func_name = func.sig.ident.to_string();
    let expected_ok_type = expected_ok_type_for_variant(state_variant)?;
    let expected_ok_display = expected_ok_type.to_token_stream().to_string();

    let ReturnType::Type(_, return_ty) = &func.sig.output else {
        return Err(quote! {
            compile_error!(concat!(
                "Error: ", #func_name, " must return `Result<", #expected_ok_display, ", _>` (or an equivalent alias)"
            ));
        });
    };

    let actual_return_type = return_ty.to_token_stream().to_string();
    let actual_ok_ty = match extract_result_ok_type(return_ty) {
        Some(ty) => ty,
        None => {
            return Err(quote! {
                compile_error!(concat!(
                    "Error: ", #func_name, " must return a `Result` type with payload `",
                    #expected_ok_display,
                    "`. Supported forms: `Result<T, E>`, `core::result::Result<T, E>`, `std::result::Result<T, E>`, and aliases like `statum::Result<T>`."
                ));
            });
        }
    };

    if !types_equivalent(&actual_ok_ty, &expected_ok_type) {
        let actual_ok_display = actual_ok_ty.to_token_stream().to_string();
        return Err(quote! {
            compile_error!(concat!(
                "Error: ", #func_name, " must return `Result<", #expected_ok_display, ", _>` (or an equivalent alias) but found `", #actual_return_type, "` with payload `", #actual_ok_display, "`"
            ));
        });
    }

    Ok(())
}

fn generate_validator_check(
    machine_ident: &Ident,
    superstate_ident: &Ident,
    field_names: &[Ident],
    state_variant: &VariantInfo,
    is_async: bool,
) -> proc_macro2::TokenStream {
    let variant_ident = format_ident!("{}", state_variant.name);
    let validator_fn_ident = format_ident!("is_{}", to_snake_case(&state_variant.name));
    let await_token = if is_async { quote! { .await } } else { quote! {} };
    let field_builder_chain = quote! { #(.#field_names(#field_names.clone()))* };

    if state_variant.data_type.is_some() {
        let builder_call = quote! {
            #machine_ident::<#variant_ident>::builder()
                #field_builder_chain
                .state_data(data)
                .build()
        };
        quote! {
            if let Ok(data) = self.#validator_fn_ident(#(&#field_names),*)#await_token {
                return Ok(#superstate_ident::#variant_ident(
                    #builder_call
                ));
            }
        }
    } else {
        let builder_call = quote! {
            #machine_ident::<#variant_ident>::builder()
                #field_builder_chain
                .build()
        };
        quote! {
            if self.#validator_fn_ident(#(&#field_names),*)#await_token.is_ok() {
                return Ok(#superstate_ident::#variant_ident(
                    #builder_call
                ));
            }
        }
    }
}

pub fn batch_builder_implementation(
    machine_ident: &Ident,
    struct_ident: &Type,
    superstate_ident: &Ident,
    machine_info: &MachineInfo,
    async_token: proc_macro2::TokenStream,
    machine_vis: syn::Visibility,
) -> proc_macro2::TokenStream {
    let trait_name_ident = format_ident!("{}BuilderExt", machine_ident);
    let builder_ident = format_ident!("{}BatchBuilder", machine_ident);
    let bon_builder_ident = format_ident!("{}Builder", builder_ident); // ✅ bon-generated builder type
    let builder_module_name = format_ident!("{}", to_snake_case(&bon_builder_ident.to_string()));

    // Extract field info
    let fields_with_types = match machine_info.fields_with_types() {
        Ok(fields) => fields,
        Err(err) => return err.to_compile_error(),
    };
    let field_names = machine_info.field_names();
    let field_builder_chain = quote! { #(.#field_names(self.#field_names.clone()))* };

    let await_token = async_token
        .is_empty()
        .then(|| quote! {})
        .unwrap_or(quote! { .await });

    let implementation = generate_finalization_logic(&field_builder_chain, &async_token);

    quote! {
        // ✅ Trait to enable batch building
        #machine_vis trait #trait_name_ident {
             fn machines_builder(self) -> #bon_builder_ident<#builder_module_name::SetItems>;
        }

        // ✅ Implement trait for anything convertible into Vec<#struct_ident>
        impl<T> #trait_name_ident for T
        where
            T: Into<Vec<#struct_ident>>,  // ✅ Works for Vec<T> AND slices
        {
            fn machines_builder(self) -> #bon_builder_ident<#builder_module_name::SetItems> {
                #builder_ident::builder().items(self.into())  // ✅ Moves Vec<T> without Clone
            }
        }

        #[derive(statum::bon::Builder)]
        #[builder(crate = ::statum::bon, finish_fn = __private_build)]
        struct #builder_ident {
            #[builder(default)]
            items: Vec<#struct_ident>,  // ✅ Now only stores Vec<T>
            #(#fields_with_types),*
        }

        // ✅ Extension method to avoid `.build().finalize()` chaining
        impl<S> #bon_builder_ident<S>
        where
            S: #builder_module_name::IsComplete, // ✅ Ensures required fields are set
        {
            #[inline(always)]
            pub #async_token fn build(self) -> Vec<core::result::Result<#superstate_ident, statum::Error>> {
                self.__private_build().__private_finalize()#await_token
            }
        }

        // ✅ Finalization logic for batch processing
        impl #builder_ident {
            #async_token fn __private_finalize(self) -> Vec<core::result::Result<#superstate_ident, statum::Error>> {
                #implementation
            }
        }
    }
}

/// Generates finalization logic for the builder
fn generate_finalization_logic(
    field_builder_chain: &proc_macro2::TokenStream,
    async_token: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    if async_token.is_empty() {
        quote! {
            self.items
                .into_iter()
                .map(|data| {
                    data.into_machine()
                        #field_builder_chain
                        .build()
                })
                .collect()
        }
    } else {
        quote! {
            futures::future::join_all(
                self.items.iter().map(|data| {
                    data.into_machine()
                        #field_builder_chain
                        .build()
                })
            ).await
        }
    }
}

use syn::{ImplItem, ImplItemFn};

/// Rewrites `is_*` methods to include machine fields as additional parameters.
fn inject_machine_fields(
    methods: &[ImplItem],
    machine_info: &MachineInfo,
) -> Result<Vec<ImplItem>, proc_macro2::TokenStream> {
    let field_idents: Vec<Ident> = machine_info.field_names();
    let mut field_types: Vec<syn::Type> = Vec::with_capacity(machine_info.fields.len());
    for field in &machine_info.fields {
        let parsed = match syn::parse_str::<syn::Type>(&field.field_type) {
            Ok(ty) => ty,
            Err(err) => return Err(err.to_compile_error()),
        };
        field_types.push(parsed);
    }

    Ok(methods
        .iter()
        .map(|item| {
            if let ImplItem::Fn(func) = item {
                let fn_name = &func.sig.ident;

                if validator_state_name_from_ident(fn_name).is_some() {
                    let mut new_inputs = func.sig.inputs.clone();

                    // Inject machine fields as `&` references
                    for (ident, ty) in field_idents.iter().zip(field_types.iter()) {
                        new_inputs.push(syn::FnArg::Typed(syn::parse_quote! { #ident: &#ty }));
                    }

                    let mut attrs = func.attrs.clone();
                    attrs.push(syn::parse_quote!(#[allow(clippy::ptr_arg)]));
                    let body = &func.block;

                    // Rebuild the method with new parameters
                    return ImplItem::Fn(ImplItemFn {
                        attrs,
                        sig: syn::Signature {
                            inputs: new_inputs,
                            ..func.sig.clone()
                        },
                        block: body.clone(),
                        ..func.clone()
                    });
                }
            }
            item.clone() // Keep other methods unchanged
        })
        .collect())
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

    // Accept either:
    // - Result<T, E> style (2 type args)
    // - statum::Result<T> style aliases (1 type arg)
    if type_args.is_empty() || type_args.len() > 2 || type_args.len() != args.args.len() {
        return None;
    }

    type_args.first().cloned()
}

fn types_equivalent(left: &Type, right: &Type) -> bool {
    match (left, right) {
        (Type::Array(a), Type::Array(b)) => {
            types_equivalent(&a.elem, &b.elem) && expr_equivalent(&a.len, &b.len)
        }
        (Type::Group(a), Type::Group(b)) => types_equivalent(&a.elem, &b.elem),
        (Type::Infer(_), Type::Infer(_)) => true,
        (Type::Never(_), Type::Never(_)) => true,
        (Type::Paren(a), Type::Paren(b)) => types_equivalent(&a.elem, &b.elem),
        (Type::Path(a), Type::Path(b)) => {
            qself_equivalent(a.qself.as_ref(), b.qself.as_ref()) && path_equivalent(&a.path, &b.path)
        }
        (Type::Ptr(a), Type::Ptr(b)) => {
            a.mutability.is_some() == b.mutability.is_some() && types_equivalent(&a.elem, &b.elem)
        }
        (Type::Reference(a), Type::Reference(b)) => {
            a.mutability.is_some() == b.mutability.is_some()
                && lifetime_equivalent(a.lifetime.as_ref(), b.lifetime.as_ref())
                && types_equivalent(&a.elem, &b.elem)
        }
        (Type::Slice(a), Type::Slice(b)) => types_equivalent(&a.elem, &b.elem),
        (Type::Tuple(a), Type::Tuple(b)) => {
            a.elems.len() == b.elems.len()
                && a.elems
                    .iter()
                    .zip(b.elems.iter())
                    .all(|(left_elem, right_elem)| types_equivalent(left_elem, right_elem))
        }
        _ => false,
    }
}

fn qself_equivalent(left: Option<&syn::QSelf>, right: Option<&syn::QSelf>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            left.position == right.position
                && types_equivalent(&left.ty, &right.ty)
                && left
                    .as_token
                    .is_some()
                    == right.as_token.is_some()
        }
        (None, None) => true,
        _ => false,
    }
}

fn path_equivalent(left: &syn::Path, right: &syn::Path) -> bool {
    if left.leading_colon.is_some() != right.leading_colon.is_some() {
        return false;
    }
    if left.segments.len() != right.segments.len() {
        return false;
    }

    left.segments.iter().zip(right.segments.iter()).all(
        |(left_segment, right_segment)| {
            left_segment.ident == right_segment.ident
                && path_arguments_equivalent(&left_segment.arguments, &right_segment.arguments)
        },
    )
}

fn path_arguments_equivalent(left: &PathArguments, right: &PathArguments) -> bool {
    match (left, right) {
        (PathArguments::None, PathArguments::None) => true,
        (PathArguments::Parenthesized(left), PathArguments::Parenthesized(right)) => {
            left.inputs.len() == right.inputs.len()
                && left
                    .inputs
                    .iter()
                    .zip(right.inputs.iter())
                    .all(|(left_ty, right_ty)| types_equivalent(left_ty, right_ty))
                && match (&left.output, &right.output) {
                    (ReturnType::Default, ReturnType::Default) => true,
                    (ReturnType::Type(_, left_ty), ReturnType::Type(_, right_ty)) => {
                        types_equivalent(left_ty, right_ty)
                    }
                    _ => false,
                }
        }
        (PathArguments::AngleBracketed(left), PathArguments::AngleBracketed(right)) => {
            left.args.len() == right.args.len()
                && left
                    .args
                    .iter()
                    .zip(right.args.iter())
                    .all(|(left_arg, right_arg)| {
                        generic_argument_equivalent(left_arg, right_arg)
                    })
        }
        _ => false,
    }
}

fn generic_argument_equivalent(left: &GenericArgument, right: &GenericArgument) -> bool {
    match (left, right) {
        (GenericArgument::Lifetime(left), GenericArgument::Lifetime(right)) => left == right,
        (GenericArgument::Type(left), GenericArgument::Type(right)) => {
            types_equivalent(left, right)
        }
        (GenericArgument::Const(left), GenericArgument::Const(right)) => {
            expr_equivalent(left, right)
        }
        (GenericArgument::AssocType(left), GenericArgument::AssocType(right)) => {
            left.ident == right.ident
                && optional_angle_generics_equivalent(&left.generics, &right.generics)
                && types_equivalent(&left.ty, &right.ty)
        }
        (GenericArgument::AssocConst(left), GenericArgument::AssocConst(right)) => {
            left.ident == right.ident
                && optional_angle_generics_equivalent(&left.generics, &right.generics)
                && expr_equivalent(&left.value, &right.value)
        }
        (GenericArgument::Constraint(left), GenericArgument::Constraint(right)) => {
            left.ident == right.ident
                && optional_angle_generics_equivalent(&left.generics, &right.generics)
                && left.bounds.len() == right.bounds.len()
                && left
                    .bounds
                    .iter()
                    .zip(right.bounds.iter())
                    .all(|(left_bound, right_bound)| token_text(left_bound) == token_text(right_bound))
        }
        _ => false,
    }
}

fn expr_equivalent(left: &syn::Expr, right: &syn::Expr) -> bool {
    token_text(left) == token_text(right)
}

fn lifetime_equivalent(left: Option<&syn::Lifetime>, right: Option<&syn::Lifetime>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left == right,
        (None, None) => true,
        _ => false,
    }
}

fn optional_angle_generics_equivalent(
    left: &Option<syn::AngleBracketedGenericArguments>,
    right: &Option<syn::AngleBracketedGenericArguments>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            left.args.len() == right.args.len()
                && left
                    .args
                    .iter()
                    .zip(right.args.iter())
                    .all(|(left_arg, right_arg)| {
                        generic_argument_equivalent(left_arg, right_arg)
                    })
        }
        (None, None) => true,
        _ => false,
    }
}

fn token_text<T: ToTokens>(value: &T) -> String {
    value.to_token_stream().to_string()
}
