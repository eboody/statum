use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote};
use std::collections::HashSet;
use syn::{FnArg, Ident, ItemImpl, ReturnType, Type, parse_macro_input};

use crate::{
    MachineInfo, MachinePath, VariantInfo, get_state_enum_variant, read_machine_map,
    read_state_enum_map, to_snake_case,
};

fn has_validators(item: &ItemImpl, state_variants: Vec<VariantInfo>) -> proc_macro2::TokenStream {
    if item.items.is_empty() {
        return quote! {};
    }

    for variant in state_variants {
        let variant_name = to_snake_case(&variant.name);
        let has_validator = item.items.iter().any(|item| {
            if let syn::ImplItem::Fn(func) = item {
                let func_name = func.sig.ident.to_string();

                func_name.starts_with("is_") && func_name.ends_with(&variant_name)
            } else {
                false
            }
        });

        if !has_validator {
            return quote! {
                compile_error!(concat!("Error: missing validator `is_", #variant_name , "`"));
            };
        }
    }

    quote! {}
}

pub fn parse_validators(attr: TokenStream, item: TokenStream, module_path: &str) -> TokenStream {
    let machine_ident = parse_macro_input!(attr as Ident);
    let item_impl = parse_macro_input!(item as ItemImpl);
    let struct_ident = &item_impl.clone().self_ty;

    let methods = item_impl.items.clone();
    let modified_methods = inject_machine_fields(&methods, &module_path.into());

    // Ensure machine metadata exists
    let machine_metadata = get_machine_metadata(&module_path.into());
    if machine_metadata.is_none() {
        return quote! {
            compile_error!("Error: No `Machine` found in scope. Ensure `#[validators(Machine)]` references a valid machine.");
        }
        .into();
    }
    let machine_metadata = machine_metadata.expect("Machine metadata not found");

    let state_enum_map = read_state_enum_map();
    let state_enum_info = state_enum_map
        .get(&module_path.into())
        .expect("State enum not found");

    let has_validators = has_validators(&item_impl, state_enum_info.variants.clone());

    let mut found_validators = HashSet::new();
    let mut validator_checks = vec![];
    let mut has_async = false;

    let field_names = machine_metadata.field_names();

    let fields_with_types = machine_metadata.fields_with_types();

    let superstate_ident = format_ident!("{}SuperState", machine_ident);

    if item_impl.items.is_empty() {
        return quote! {
            compile_error!("Error: No validator functions found in impl block");
        }
        .into();
    }

    // Generate validator checks
    for item in &item_impl.items {
        if let syn::ImplItem::Fn(func) = item {
            let func_name = func.sig.ident.to_string();
            if func_name.starts_with("is_") {
                let state_name = func_name
                    .strip_prefix("is_")
                    .expect("Invalid function name")
                    .to_string();
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
                let state_variant = get_state_enum_variant(&module_path.into(), &state_name);

                if let Some(state_variant) = state_variant {
                    let expected_return_type = match &state_variant.data_type {
                        Some(data_type) => format!("Result<{}", data_type),
                        None => "Result<()".to_string(),
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

                        if !actual_return_type.starts_with(&expected_return_type) {
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

                    let field_builder_chain = quote! { #(.#field_names(#field_names.clone()))* };

                    if state_variant.data_type.is_some() {
                        let builder_call = quote! {
                            #machine_ident::<#variant_ident>::builder()
                                #field_builder_chain
                                .state_data(data)
                                .build()
                        };
                        // If state has data
                        validator_checks.push(quote! {
                            if let Ok(data) = self.#validator_fn_ident(#(&#field_names),*)#await_token {
                                return Ok(#superstate_ident::#variant_ident(
                                    #builder_call
                                ));
                            }
                        });
                    } else {
                        let builder_call = quote! {
                            #machine_ident::<#variant_ident>::builder()
                                #field_builder_chain
                                .build()
                        };
                        // If state has NO data
                        validator_checks.push(quote! {
                            if self.#validator_fn_ident(#(&#field_names),*)#await_token.is_ok() {
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
    let superstate_variants = state_enum_info.variants.iter().map(|variant| {
        let variant_ident = format_ident!("{}", variant.name);
        quote! {
            #variant_ident(#machine_ident<#variant_ident>)
        }
    });

    let superstate_enum = quote! {
        pub enum #superstate_ident {
            #(#superstate_variants),*
        }
    };

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
            #(#modified_methods)*
        }

        #batch_builder_impl
    };

    // For each variant, create `is_{variant_name}(&self) -> bool`.
    let is_methods = state_enum_info.variants.iter().map(|variant| {
        let variant_ident = format_ident!("{}", variant.name);
        let fn_name = format_ident!("is_{}", crate::to_snake_case(&variant.name));
        quote! {
            pub fn #fn_name(&self #(, #fields_with_types)*) -> bool {
                matches!(self, #superstate_ident::#variant_ident(_))
            }
        }
    });

    let superstate_impl = quote! {
        impl #superstate_ident {
            #(#is_methods)*
        }
    };

    // Merge original item with generated code
    let expanded = quote! {
        #has_validators
        #superstate_enum
        #superstate_impl
        #machine_builder_impl
    };

    expanded.into()
}

pub fn get_machine_metadata(machine_path: &MachinePath) -> Option<MachineInfo> {
    let machine_map = read_machine_map();
    machine_map.get(machine_path).cloned()
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
    let fields_with_types = machine_info.fields_with_types();
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
        #[builder(finish_fn = __private_build)]
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
                    data.machine_builder()
                        #field_builder_chain
                        .build()
                })
                .collect()
        }
    } else {
        quote! {
            futures::future::join_all(
                self.items.iter().map(|data| {
                    data.machine_builder()
                        #field_builder_chain
                        .build()
                })
            ).await
        }
    }
}

use syn::{ImplItem, ImplItemFn};

/// Rewrites `is_*` methods to include machine fields as additional parameters.
fn inject_machine_fields(methods: &[ImplItem], machine_path: &MachinePath) -> Vec<ImplItem> {
    let machine_map = read_machine_map();

    // Retrieve machine metadata
    let machine_info = match machine_map.get(machine_path) {
        Some(info) => info,
        None => panic!("MachinePath '{}' not found in registry", machine_path.0),
    };

    let field_idents: Vec<Ident> = machine_info.field_names();
    let field_types: Vec<syn::Type> = machine_info
        .fields
        .iter()
        .map(|field| {
            let field_type = turn_string_ref_into_str_slice(&field.field_type);
            syn::parse_str::<syn::Type>(field_type).expect("Failed to parse field type")
        })
        .collect();

    methods
        .iter()
        .map(|item| {
            if let ImplItem::Fn(func) = item {
                let fn_name = &func.sig.ident;

                if fn_name.to_string().starts_with("is_") {
                    let mut new_inputs = func.sig.inputs.clone();

                    // Inject machine fields as `&` references
                    for (ident, ty) in field_idents.iter().zip(field_types.iter()) {
                        new_inputs.push(syn::FnArg::Typed(syn::parse_quote! { #ident: &#ty }));
                    }

                    let _asyncness = &func.sig.asyncness;
                    let _output = &func.sig.output;
                    let body = &func.block;

                    // Rebuild the method with new parameters
                    return ImplItem::Fn(ImplItemFn {
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
        .collect()
}

fn turn_string_ref_into_str_slice(input: &str) -> &str {
    if input == "String" { "str" } else { input }
}
