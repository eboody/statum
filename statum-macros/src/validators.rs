use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use std::collections::HashSet;
use syn::{parse_macro_input, FnArg, Ident, ItemImpl, ReturnType, Type};

use crate::{
    get_state_enum_variant, read_machine_map, read_state_enum_map, to_snake_case, MachineInfo,
    MachinePath,
};

use proc_macro::Span;

pub fn parse_validators(attr: TokenStream, item: TokenStream) -> TokenStream {
    let path = Span::call_site().source_file().path();
    let file_path = path.to_str().unwrap();

    let machine_ident = parse_macro_input!(attr as Ident);
    let item_impl = parse_macro_input!(item as ItemImpl);
    let struct_ident = &item_impl.clone().self_ty;

    let methods = item_impl.items.clone();

    // Ensure machine metadata exists
    let machine_metadata = get_machine_metadata(&file_path.into());
    if machine_metadata.is_none() {
        return quote! {
            compile_error!("Error: No `Machine` found in scope. Ensure `#[validators(Machine)]` references a valid machine.");
        }
        .into();
    }
    let machine_metadata = machine_metadata.unwrap();

    let state_enum_map = read_state_enum_map();
    let state_enum_info = state_enum_map
        .get(&file_path.into())
        .expect("State enum not found");

    let mut found_validators = HashSet::new();
    let mut validator_checks = vec![];
    let mut has_async = false;

    let field_names = machine_metadata.field_names();

    let fields_with_types = machine_metadata.fields_with_types();

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
                let state_variant = get_state_enum_variant(&file_path.into(), &state_name);

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
                            if let Ok(data) = self.#validator_fn_ident()#await_token {
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
            #machine_vis #async_token fn new(&self, #(#fields_with_types),*) -> core::result::Result<#superstate_ident, statum::Error> {
                #(#validator_checks)*

                Err(statum::Error::InvalidState)
            }
            #(#methods)*
        }

        #batch_builder_impl
    };

    // For each variant, create `is_{variant_name}(&self) -> bool`.
    let is_methods = state_enum_info.variants.iter().map(|variant| {
        let variant_ident = format_ident!("{}", variant.name);
        let fn_name = format_ident!("is_{}", crate::to_snake_case(&variant.name));
        quote! {
            pub fn #fn_name(&self) -> bool {
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
    let bon_builder_ident = format_ident!("{}Builder", builder_ident); // ✅ The actual bon-generated builder type
    let builder_module_name = format_ident!("{}", to_snake_case(&bon_builder_ident.to_string()));

    let fields_with_types = machine_info.fields_with_types();
    let field_names = machine_info.field_names();
    let field_builder_chain = quote! { #(.#field_names(self.#field_names.clone()))* };

    let implementation = if async_token.is_empty() {
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
    };

    let trait_impl = quote! {
        trait #trait_name_ident {
            #machine_vis fn build_machines(&self) -> #bon_builder_ident<#builder_module_name::SetItems>;
        }

        impl #trait_name_ident for [#struct_ident] {
            #machine_vis fn build_machines(&self) -> #bon_builder_ident<#builder_module_name::SetItems> {
                let items = self.to_vec();
                #builder_ident::builder().items(items)
            }
        }

        #[derive(statum::bon::Builder, Clone)]
        struct #builder_ident {
            #[builder(default)]
            items: Vec<#struct_ident>,
            #(#fields_with_types),*
        }

        impl #builder_ident {
            #machine_vis #async_token fn finalize(self) -> Vec<core::result::Result<#superstate_ident, statum::Error>> {
                #implementation
            }
        }
    };

    trait_impl
}
