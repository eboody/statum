use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Generics, Ident, Visibility};

use crate::machine::{
    MachineInfo, builder_generics, extra_generics, extra_type_arguments_tokens,
    generic_argument_tokens, machine_type_with_state,
};
use crate::state::{ParsedEnumInfo, ParsedVariantInfo, ParsedVariantShape};

use super::super::metadata::{ParsedMachineInfo, is_rust_analyzer};

impl MachineInfo {
    pub fn generate_builder_methods(
        &self,
        parsed_machine: &ParsedMachineInfo,
        parsed_state: &ParsedEnumInfo,
    ) -> TokenStream {
        let parsed_fields = parsed_machine.field_idents_and_types();
        let field_names = parsed_fields
            .iter()
            .map(|(field_ident, _)| field_ident.clone())
            .collect::<Vec<_>>();
        let field_types = parsed_fields
            .iter()
            .map(|(_, field_ty)| field_ty.clone())
            .collect::<Vec<_>>();

        let machine_ident = format_ident!("{}", self.name);
        let builder_context = BuilderContext {
            machine_ident: &machine_ident,
            machine_generics: &parsed_machine.generics,
            builder_vis: &parsed_machine.vis,
            field_names: &field_names,
            field_types: &field_types,
            use_ra_shim: is_rust_analyzer(),
        };
        let builder_methods = parsed_state
            .variants
            .iter()
            .map(|variant| generate_variant_builder_tokens(&builder_context, variant));

        quote! {
            #(#builder_methods)*
        }
    }
}

struct BuilderContext<'a> {
    machine_ident: &'a Ident,
    machine_generics: &'a Generics,
    builder_vis: &'a Visibility,
    field_names: &'a [Ident],
    field_types: &'a [syn::Type],
    use_ra_shim: bool,
}

fn generate_variant_builder_tokens(
    context: &BuilderContext<'_>,
    variant: &ParsedVariantInfo,
) -> TokenStream {
    let variant_ident = format_ident!("{}", variant.name);
    let variant_builder_ident = format_ident!("{}{}Builder", context.machine_ident, variant.name);
    let data_type = variant_payload_type(variant);
    generate_custom_builder_tokens(
        context,
        &variant_ident,
        &variant_builder_ident,
        data_type.as_ref(),
    )
}

fn generate_custom_builder_tokens(
    context: &BuilderContext<'_>,
    variant_ident: &Ident,
    variant_builder_ident: &Ident,
    data_type: Option<&syn::Type>,
) -> TokenStream {
    let machine_ident = context.machine_ident;
    let machine_generics = context.machine_generics;
    let builder_vis = context.builder_vis;
    let field_names = context.field_names;
    let field_types = context.field_types;
    let extra_generics = extra_generics(machine_generics);
    let extra_ty_args = extra_type_arguments_tokens(machine_generics);
    let (extra_impl_generics, _, extra_where_clause) = extra_generics.split_for_impl();
    let machine_state_ty = machine_type_with_state(
        quote! { #machine_ident },
        machine_generics,
        quote! { #variant_ident },
    );
    let struct_initialization = machine_struct_initialization(context, data_type.is_some());

    if context.use_ra_shim {
        let builder_generics = extra_generics.clone();
        let (builder_impl_generics, builder_ty_generics, builder_where_clause) =
            builder_generics.split_for_impl();
        let state_data_method = data_type.map(|parsed_data_type| {
            quote! {
                #builder_vis fn state_data(self, _data: #parsed_data_type) -> Self {
                    self
                }
            }
        });

        return quote! {
            #builder_vis struct #variant_builder_ident #builder_generics;

            impl #builder_impl_generics #variant_builder_ident #builder_ty_generics #builder_where_clause {
                #state_data_method
                #(#builder_vis fn #field_names(self, _value: #field_types) -> Self { self })*

                #builder_vis fn build(self) -> #machine_state_ty {
                    panic!("statum rust-analyzer shim: builder values are not constructed at runtime")
                }
            }

            impl #extra_impl_generics #machine_state_ty #extra_where_clause {
                #builder_vis fn builder() -> #variant_builder_ident #extra_ty_args {
                    #variant_builder_ident
                }
            }
        };
    }

    let has_state_data = data_type.is_some();
    let slot_types = data_type
        .into_iter()
        .cloned()
        .chain(field_types.iter().cloned())
        .collect::<Vec<_>>();
    let slot_storage_idents = (0..slot_types.len())
        .map(|idx| format_ident!("__statum_slot_{}", idx))
        .collect::<Vec<_>>();
    let slot_state_idents = (0..slot_types.len())
        .map(|idx| format_ident!("__STATUM_SLOT_{}_SET", idx))
        .collect::<Vec<_>>();
    let struct_fields = slot_storage_idents
        .iter()
        .zip(slot_types.iter())
        .map(|(storage_ident, slot_type)| {
            quote! { #storage_ident: core::option::Option<#slot_type> }
        })
        .collect::<Vec<_>>();
    let builder_defaults = builder_generics(&extra_generics, false, &slot_state_idents, true);
    let builder_init = slot_storage_idents.iter().map(|storage_ident| {
        quote! { #storage_ident: core::option::Option::None }
    });
    let complete_builder_ty_generics = {
        let complete = slot_state_idents
            .iter()
            .map(|_| quote! { true })
            .collect::<Vec<_>>();
        generic_argument_tokens(extra_generics.params.iter(), None, &complete)
    };
    let initial_builder_ty_generics = {
        let initial = slot_state_idents
            .iter()
            .map(|_| quote! { false })
            .collect::<Vec<_>>();
        generic_argument_tokens(extra_generics.params.iter(), None, &initial)
    };
    let state_data_binding = if has_state_data {
        let storage_ident = &slot_storage_idents[0];
        Some(quote! {
            let state_data = self.#storage_ident.expect(
                "statum internal error: `state_data` was not set before build",
            );
        })
    } else {
        None
    };
    let field_bindings = field_names.iter().enumerate().map(|(field_idx, field_name)| {
        let storage_ident = &slot_storage_idents[field_idx + usize::from(has_state_data)];
        let message = format!("statum internal error: `{field_name}` was not set before build");
        quote! {
            let #field_name = self.#storage_ident.expect(#message);
        }
    });
    let setters = slot_types.iter().enumerate().map(|(slot_idx, slot_type)| {
        let setter_ident = if has_state_data && slot_idx == 0 {
            format_ident!("state_data")
        } else {
            field_names[slot_idx - usize::from(has_state_data)].clone()
        };
        let available_slot_idents = slot_state_idents
            .iter()
            .enumerate()
            .filter_map(|(idx, ident)| (idx != slot_idx).then_some(ident.clone()))
            .collect::<Vec<_>>();
        let setter_impl_generics_decl =
            builder_generics(&extra_generics, false, &available_slot_idents, false);
        let (setter_impl_generics, _, setter_where_clause) =
            setter_impl_generics_decl.split_for_impl();
        let current_generics = slot_state_idents
            .iter()
            .enumerate()
            .map(|(idx, ident)| {
                if idx == slot_idx {
                    quote! { false }
                } else {
                    quote! { #ident }
                }
            })
            .collect::<Vec<_>>();
        let current_ty_generics =
            generic_argument_tokens(extra_generics.params.iter(), None, &current_generics);
        let target_generics = if slot_state_idents.is_empty() {
            extra_ty_args.clone()
        } else {
            let generics = slot_state_idents
                .iter()
                .enumerate()
                .map(|(idx, ident)| {
                    if idx == slot_idx {
                        quote! { true }
                    } else {
                        quote! { #ident }
                    }
                })
                .collect::<Vec<_>>();
            generic_argument_tokens(extra_generics.params.iter(), None, &generics)
        };
        let assignments = slot_storage_idents.iter().enumerate().map(|(idx, storage_ident)| {
            if idx == slot_idx {
                quote! { #storage_ident: core::option::Option::Some(value) }
            } else {
                quote! { #storage_ident: self.#storage_ident }
            }
        });
        quote! {
            impl #setter_impl_generics #variant_builder_ident #current_ty_generics #setter_where_clause {
                #builder_vis fn #setter_ident(self, value: #slot_type) -> #variant_builder_ident #target_generics {
                    #variant_builder_ident {
                        #(#assignments),*
                    }
                }
            }
        }
    });

    quote! {
        #builder_vis struct #variant_builder_ident #builder_defaults {
            #(#struct_fields),*
        }

        impl #extra_impl_generics #machine_state_ty #extra_where_clause {
            #builder_vis fn builder() -> #variant_builder_ident #initial_builder_ty_generics {
                #variant_builder_ident {
                    #(#builder_init),*
                }
            }
        }
        #(#setters)*

        impl #extra_impl_generics #variant_builder_ident #complete_builder_ty_generics #extra_where_clause {
            #builder_vis fn build(self) -> #machine_state_ty {
                #state_data_binding
                #(#field_bindings)*
                #struct_initialization
            }
        }
    }
}

fn variant_payload_type(variant: &ParsedVariantInfo) -> Option<syn::Type> {
    match &variant.shape {
        ParsedVariantShape::Unit => None,
        ParsedVariantShape::Tuple { data_type } => Some(*data_type.clone()),
        ParsedVariantShape::Named {
            data_struct_ident, ..
        } => Some(syn::parse_quote!(#data_struct_ident)),
    }
}

fn machine_struct_initialization(
    context: &BuilderContext<'_>,
    has_state_data: bool,
) -> TokenStream {
    let machine_ident = context.machine_ident;
    let field_names = context.field_names;
    let state_data = if has_state_data {
        quote! { state_data }
    } else {
        quote! { state_data: () }
    };

    if !field_names.is_empty() {
        quote! {
            #machine_ident {
                marker: core::marker::PhantomData,
                #state_data,
                #(#field_names,)*
            }
        }
    } else {
        quote! {
            #machine_ident {
                marker: core::marker::PhantomData,
                #state_data,
            }
        }
    }
}
