use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use std::collections::HashSet;
use syn::{parse_macro_input, FnArg, Ident, ItemImpl, ReturnType, Type};

use crate::{
    get_state_enum_variant, read_machine_map, read_state_enum_map, to_snake_case, MachineInfo,
    MachinePath,
};

pub fn parse_validators(attr: TokenStream, item: TokenStream) -> TokenStream {
    let file_path: String = std::env::current_dir()
        .expect("Failed to get current directory.")
        .to_string_lossy()
        .to_string();

    let machine_ident = parse_macro_input!(attr as Ident);
    let item_impl = parse_macro_input!(item as ItemImpl);
    let struct_ident = &item_impl.clone().self_ty;

    let methods = item_impl.items.clone();

    // Ensure machine metadata exists
    let machine_metadata = get_machine_metadata(&file_path.clone().into());
    if machine_metadata.is_none() {
        return quote! {
            compile_error!("Error: No `Machine` found in scope. Ensure `#[validators(Machine)]` references a valid machine.");
        }
        .into();
    }
    let machine_metadata = machine_metadata.unwrap();

    let state_enum_map = read_state_enum_map();
    let state_enum_info = state_enum_map
        .get(&file_path.clone().into())
        .expect("State enum not found");

    let mut found_validators = HashSet::new();
    let mut validator_checks = vec![];
    let mut has_async = false;

    let field_names = machine_metadata
        .fields
        .iter()
        .map(|field| format_ident!("{}", field.name))
        .collect::<Vec<_>>();

    let fields_map = machine_metadata
        .fields
        .iter()
        .map(|field| {
            let field_ident = format_ident!("{}", field.name);
            let field_ty = syn::parse_str::<syn::Type>(&field.field_type).unwrap();
            quote! { #field_ident: #field_ty }
        })
        .collect::<Vec<_>>();

    let superstate_ident = format_ident!("{}SuperState", machine_ident);

    // Generate validator checks
    for item in &item_impl.items {
        if let syn::ImplItem::Fn(func) = item {
            let func_name = func.sig.ident.to_string();
            if func_name.starts_with("is_") {
                let state_name = func_name.strip_prefix("is_").unwrap().to_string();
                found_validators.insert(state_name.clone());

                // Ensure correct function signature
                if func.sig.inputs.len() != 1 {
                    return quote! {
                        compile_error!(concat!("Error: ", #func_name, " must take exactly one argument: `&self`"));
                    }
                    .into();
                }
                if let FnArg::Typed(arg) = &func.sig.inputs[0] {
                    if !matches!(*arg.ty, Type::Reference(_)) {
                        return quote! {
                            compile_error!(concat!("Error: ", #func_name, " must take `&self` as the first argument"));
                        }
                        .into();
                    }
                }

                // Get expected return type based on state variant data
                let state_variant = get_state_enum_variant(&file_path.clone().into(), &state_name);

                if let Some(state_variant) = state_variant {
                    let expected_return_type = match &state_variant.data_type {
                        Some(data_type) => format!("Result<{}>", data_type),
                        None => "Result<()>".to_string(),
                    };

                    // Check if the function is async
                    let is_async = func.sig.asyncness.is_some();
                    if is_async {
                        has_async = true;
                    }

                    // Validate function return type
                    if let ReturnType::Type(_, return_ty) = &func.sig.output {
                        let actual_return_type =
                            return_ty.to_token_stream().to_string().replace(" ", "");
                        if actual_return_type != expected_return_type {
                            return quote! {
                                compile_error!(concat!(
                                    "Error: ", #func_name, " must return `", #expected_return_type, "` but found `", #actual_return_type, "`"
                                ));
                            }
                            .into();
                        }
                    } else {
                        return quote! {
                            compile_error!(concat!(
                                "Error: ", #func_name, " must return `", #expected_return_type, "`"
                            ));
                        }
                        .into();
                    }

                    // Generate validator check inside `new()`
                    let variant_ident = format_ident!("{}", state_variant.name);
                    let validator_fn_ident =
                        format_ident!("is_{}", to_snake_case(&state_variant.name));

                    let await_token = if is_async {
                        quote! { .await }
                    } else {
                        quote! {}
                    };

                    let builder_call = quote! {
                        #machine_ident::<#variant_ident>::builder()
                            #(.#field_names(#field_names.clone()))*
                            .build()
                    };

                    if state_variant.data_type.is_some() {
                        // If state has data
                        validator_checks.push(quote! {
                            if let Ok(data) = self.#validator_fn_ident()#await_token {
                                return Ok(#superstate_ident::#variant_ident(
                                    #builder_call.transition_with(data)
                                ));
                            }
                        });
                    } else {
                        // If state has NO data
                        validator_checks.push(quote! {
                            if self.#validator_fn_ident()#await_token.is_ok() {
                                return Ok(#superstate_ident::#variant_ident(
                                    #builder_call
                                ));
                            }
                        });
                    }
                }
            }
        }
    }

    // **Generate SuperState Enum**
    let state_variants_enum = state_enum_info.variants.iter().map(|variant| {
        let variant_ident = format_ident!("{}", variant.name);
        quote! {
            #variant_ident(#machine_ident<#variant_ident>)
        }
    });

    let superstate_enum = quote! {
        pub enum #superstate_ident {
            #(#state_variants_enum),*
        }
    };

    let machine_vis = format_ident!("{}", machine_metadata.vis);
    let async_token = if has_async {
        quote! { async }
    } else {
        quote! {}
    };

    let batch_builder_impl =
        batch_builder_implementation(&machine_ident, struct_ident, &superstate_ident);

    // **Fill in `new()` with the validation logic**
    let machine_builder_impl = quote! {
        #[statum::bon::bon(crate = ::statum::bon)]
        impl #struct_ident {
            #[builder(start_fn = machine_builder)]
            #machine_vis #async_token fn new(&self, #(#fields_map),*) -> core::result::Result<#superstate_ident, statum::Error> {
                #(#validator_checks)*

                Err(statum::Error::InvalidState)
            }
            #(#methods)*
        }

        #batch_builder_impl
    };

    // Merge original item with generated code
    let expanded = quote! {
        #superstate_enum
        #machine_builder_impl
    };

    expanded.into()
}

pub fn get_machine_metadata(machine_path: &MachinePath) -> Option<MachineInfo> {
    read_machine_map().get(machine_path).cloned()
}

pub fn batch_builder_implementation(
    machine_ident: &Ident,
    struct_ident: &Type,
    superstate_ident: &Ident,
) -> proc_macro2::TokenStream {
    let trait_name_ident = format_ident!("{}BuilderExt", machine_ident);

    let implementation = quote! {

        trait #trait_name_ident {
            async fn build_machines(&self) -> Vec<#superstate_ident>;
        }

        impl #trait_name_ident for [#struct_ident] {
            async fn build_machines(&self) -> Vec<#superstate_ident> {
                futures::future::join_all(
                    self.iter()
                        .map(|data| data.machine_builder().name(data.id.clone()).build()),
                )
                    .await
                    .into_iter()
                    .filter_map(core::result::Result::ok)
                    .collect()
            }
        }
    };

    implementation
}
