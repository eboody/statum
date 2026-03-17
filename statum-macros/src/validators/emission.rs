use quote::{format_ident, quote};
use syn::{Ident, ImplItem, ImplItemFn, Type};

use crate::to_snake_case;

pub(super) struct BatchBuilderContext<'a> {
    pub(super) machine_ident: &'a Ident,
    pub(super) machine_module_ident: &'a Ident,
    pub(super) struct_ident: &'a Type,
    pub(super) machine_state_ty: &'a proc_macro2::TokenStream,
    pub(super) field_names: &'a [Ident],
    pub(super) field_types: &'a [Type],
    pub(super) async_token: proc_macro2::TokenStream,
    pub(super) machine_vis: syn::Visibility,
}

pub(super) fn generate_validator_check(
    machine_ident: &Ident,
    machine_state_ty: &proc_macro2::TokenStream,
    field_names: &[Ident],
    receiver: &proc_macro2::TokenStream,
    variant_name: &str,
    has_state_data: bool,
    is_async: bool,
) -> proc_macro2::TokenStream {
    let variant_ident = format_ident!("{}", variant_name);
    let validator_fn_ident = format_ident!("is_{}", to_snake_case(variant_name));
    let await_token = if is_async { quote! { .await } } else { quote! {} };
    let field_builder_chain = quote! { #(.#field_names(#field_names.clone()))* };

    if has_state_data {
        let builder_call = quote! {
            #machine_ident::<#variant_ident>::builder()
                #field_builder_chain
                .state_data(data)
                .build()
        };
        quote! {
            if let Ok(data) = #receiver.#validator_fn_ident(#(&#field_names),*)#await_token {
                return Ok(#machine_state_ty::#variant_ident(
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
            if #receiver.#validator_fn_ident(#(&#field_names),*)#await_token.is_ok() {
                return Ok(#machine_state_ty::#variant_ident(
                    #builder_call
                ));
            }
        }
    }
}

pub(super) fn batch_builder_implementation(
    context: BatchBuilderContext<'_>,
) -> proc_macro2::TokenStream {
    let builder_ident = format_ident!("__Statum{}IntoMachines", context.machine_ident);
    let by_builder_ident = format_ident!("__Statum{}IntoMachinesBy", context.machine_ident);
    let machine_module_ident = context.machine_module_ident;
    let struct_ident = context.struct_ident;
    let machine_state_ty = context.machine_state_ty;
    let field_names = context.field_names;
    let field_types = context.field_types;
    let async_token = context.async_token;
    let machine_vis = context.machine_vis;
    let fields_ty = quote! { #machine_module_ident::Fields };

    let field_builder_chain = quote! { #(.#field_names(#field_names.clone()))* };
    let per_item_builder_chain = quote! { #(.#field_names(fields.#field_names))* };
    let await_token = if async_token.is_empty() {
        quote! {}
    } else {
        quote! { .await }
    };

    let implementation = generate_finalization_logic(&field_builder_chain, &async_token);
    let per_item_implementation =
        generate_per_item_finalization_logic(&per_item_builder_chain, &async_token);
    let slot_state_idents = (0..field_names.len())
        .map(|idx| format_ident!("__STATUM_SLOT_{}_SET", idx))
        .collect::<Vec<_>>();
    let builder_defaults = if slot_state_idents.is_empty() {
        quote! {}
    } else {
        quote! { <#(const #slot_state_idents: bool = false),*> }
    };
    let builder_impl_generics = if slot_state_idents.is_empty() {
        quote! {}
    } else {
        quote! { <#(const #slot_state_idents: bool),*> }
    };
    let builder_ty_generics = if slot_state_idents.is_empty() {
        quote! {}
    } else {
        quote! { <#(#slot_state_idents),*> }
    };
    let complete_builder_ty_generics = if slot_state_idents.is_empty() {
        quote! {}
    } else {
        let complete = slot_state_idents.iter().map(|_| quote! { true });
        quote! { <#(#complete),*> }
    };
    let field_storage = field_names.iter().zip(field_types.iter()).map(|(field_name, field_type)| {
        quote! { #field_name: core::option::Option<#field_type> }
    });
    let builder_init = field_names.iter().map(|field_name| {
        quote! { #field_name: core::option::Option::None }
    });
    let field_bindings = field_names.iter().map(|field_name| {
        let message = format!("statum internal error: `{field_name}` was not set before build");
        quote! {
            let #field_name = self.#field_name.expect(#message);
        }
    });
    let setters = field_names
        .iter()
        .zip(field_types.iter())
        .enumerate()
        .map(|(slot_idx, (field_name, field_type))| {
            let target_generics = if slot_state_idents.is_empty() {
                quote! {}
            } else {
                let generics = slot_state_idents.iter().enumerate().map(|(idx, ident)| {
                    if idx == slot_idx {
                        quote! { true }
                    } else {
                        quote! { #ident }
                    }
                });
                quote! { <#(#generics),*> }
            };
            let assignments = field_names.iter().enumerate().map(|(idx, existing_field_name)| {
                if idx == slot_idx {
                    quote! { #existing_field_name: core::option::Option::Some(value) }
                } else {
                    quote! { #existing_field_name: self.#existing_field_name }
                }
            });

            quote! {
                #machine_vis fn #field_name(self, value: #field_type) -> #builder_ident #target_generics {
                    #builder_ident {
                        items: self.items,
                        #(#assignments),*
                    }
                }
            }
        });

    quote! {
        impl<T> #machine_module_ident::IntoMachinesExt<#struct_ident> for T
        where
            T: Into<Vec<#struct_ident>>,
        {
            type Builder = #builder_ident;
            type BuilderWithFields<F> = #by_builder_ident<F>;

            fn into_machines(self) -> Self::Builder {
                #builder_ident {
                    items: self.into(),
                    #(#builder_init),*
                }
            }

            fn into_machines_by<F>(self, fields: F) -> Self::BuilderWithFields<F>
            where
                F: Fn(&#struct_ident) -> #fields_ty,
            {
                #by_builder_ident {
                    items: self.into(),
                    fields,
                }
            }
        }

        #[doc(hidden)]
        #machine_vis struct #builder_ident #builder_defaults {
            items: Vec<#struct_ident>,
            #(#field_storage),*
        }

        impl #builder_impl_generics #builder_ident #builder_ty_generics {
            #(#setters)*
        }

        impl #builder_ident #complete_builder_ty_generics {
            #[inline(always)]
            #machine_vis #async_token fn build(self) -> Vec<core::result::Result<#machine_state_ty, statum::Error>> {
                let items = self.items;
                #(#field_bindings)*
                #implementation
            }
        }

        #[doc(hidden)]
        #machine_vis struct #by_builder_ident<F> {
            items: Vec<#struct_ident>,
            fields: F,
        }

        impl<F> #by_builder_ident<F>
        where
            F: Fn(&#struct_ident) -> #fields_ty,
        {
            #[inline(always)]
            #machine_vis #async_token fn build(self) -> Vec<core::result::Result<#machine_state_ty, statum::Error>> {
                self.__private_finalize()#await_token
            }

            #async_token fn __private_finalize(self) -> Vec<core::result::Result<#machine_state_ty, statum::Error>> {
                #per_item_implementation
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
            items
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
                items.iter().map(|data| {
                    data.into_machine()
                        #field_builder_chain
                        .build()
                })
            ).await
        }
    }
}

fn generate_per_item_finalization_logic(
    field_builder_chain: &proc_macro2::TokenStream,
    async_token: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    if async_token.is_empty() {
        quote! {
            let field_fn = self.fields;
            self.items
                .into_iter()
                .map(|data| {
                    let fields = field_fn(&data);
                    data.into_machine()
                        #field_builder_chain
                        .build()
                })
                .collect()
        }
    } else {
        quote! {
            let field_fn = &self.fields;
            futures::future::join_all(
                self.items.iter().map(|data| {
                    let fields = field_fn(data);
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
