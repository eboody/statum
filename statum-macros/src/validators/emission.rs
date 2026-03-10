use quote::{format_ident, quote};
use syn::{Ident, ImplItem, ImplItemFn, Type};

use crate::{MachineInfo, VariantInfo, to_snake_case};

pub(super) fn parsed_machine_fields(
    machine_info: &MachineInfo,
) -> Result<Vec<(Ident, Type)>, proc_macro2::TokenStream> {
    machine_info.parsed_field_idents_and_types()
}

pub(super) fn generate_validator_check(
    machine_ident: &Ident,
    superstate_ty: &proc_macro2::TokenStream,
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
                return Ok(#superstate_ty::#variant_ident(
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
                return Ok(#superstate_ty::#variant_ident(
                    #builder_call
                ));
            }
        }
    }
}

pub(super) fn batch_builder_implementation(
    machine_ident: &Ident,
    struct_ident: &Type,
    superstate_ty: &proc_macro2::TokenStream,
    fields_with_types: &[proc_macro2::TokenStream],
    field_names: &[Ident],
    async_token: proc_macro2::TokenStream,
    machine_vis: syn::Visibility,
) -> proc_macro2::TokenStream {
    let trait_name_ident = format_ident!("{}BuilderExt", machine_ident);
    let builder_ident = format_ident!("{}BatchBuilder", machine_ident);
    let bon_builder_ident = format_ident!("{}Builder", builder_ident);
    let builder_module_name = format_ident!("{}", to_snake_case(&bon_builder_ident.to_string()));

    let field_builder_chain = quote! { #(.#field_names(self.#field_names.clone()))* };

    let await_token = async_token
        .is_empty()
        .then(|| quote! {})
        .unwrap_or(quote! { .await });

    let implementation = generate_finalization_logic(&field_builder_chain, &async_token);

    quote! {
        #machine_vis trait #trait_name_ident {
             fn machines_builder(self) -> #bon_builder_ident<#builder_module_name::SetItems>;
        }

        impl<T> #trait_name_ident for T
        where
            T: Into<Vec<#struct_ident>>,
        {
            fn machines_builder(self) -> #bon_builder_ident<#builder_module_name::SetItems> {
                #builder_ident::builder().items(self.into())
            }
        }

        #[derive(statum::bon::Builder)]
        #[builder(crate = ::statum::bon, finish_fn = __private_build)]
        struct #builder_ident {
            #[builder(default)]
            items: Vec<#struct_ident>,
            #(#fields_with_types),*
        }

        impl<S> #bon_builder_ident<S>
        where
            S: #builder_module_name::IsComplete,
        {
            #[inline(always)]
            pub #async_token fn build(self) -> Vec<core::result::Result<#superstate_ty, statum::Error>> {
                self.__private_build().__private_finalize()#await_token
            }
        }

        impl #builder_ident {
            #async_token fn __private_finalize(self) -> Vec<core::result::Result<#superstate_ty, statum::Error>> {
                #implementation
            }
        }
    }
}

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

pub(super) fn inject_machine_fields(
    methods: &[ImplItem],
    parsed_fields: &[(Ident, Type)],
) -> Result<Vec<ImplItem>, proc_macro2::TokenStream> {
    Ok(methods
        .iter()
        .map(|item| {
            if let ImplItem::Fn(func) = item {
                let fn_name = &func.sig.ident;

                if super::signatures::validator_state_name_from_ident(fn_name).is_some() {
                    let mut new_inputs = func.sig.inputs.clone();

                    for (ident, ty) in parsed_fields.iter() {
                        new_inputs.push(syn::FnArg::Typed(syn::parse_quote! { #ident: &#ty }));
                    }

                    let mut attrs = func.attrs.clone();
                    attrs.push(syn::parse_quote!(#[allow(clippy::ptr_arg)]));
                    let body = &func.block;

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
            item.clone()
        })
        .collect())
}
