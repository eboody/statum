use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    GenericArgument, GenericParam, Generics, Ident, ItemStruct, LitStr, PathArguments, Type,
    TypePath,
};

use crate::state::{
    state_family_target_resolver_macro_ident, state_family_visitor_macro_ident,
};
use crate::{PresentationAttr, PresentationTypesAttr, to_snake_case};

use super::metadata::{ParsedMachineInfo, field_type_alias_name};
use super::{
    MachineInfo, builder_generics, extra_generics, extra_type_arguments_tokens,
    generic_argument_tokens,
    linked_transition_slice_ident, machine_type_with_state,
    transition_presentation_slice_ident, transition_slice_ident,
};

pub fn generate_machine_impls(
    machine_info: &MachineInfo,
    item: &ItemStruct,
) -> proc_macro2::TokenStream {
    let parsed_machine = match machine_info.parse() {
        Ok(parsed) => parsed,
        Err(err) => return err,
    };
    let machine_ident = format_ident!("{}", machine_info.name);
    let state_generic_ident = match extract_state_generic_ident(&parsed_machine.generics) {
        Ok(ident) => ident,
        Err(err) => return err,
    };
    let state_generic_name = machine_info
        .state_generic_name
        .clone()
        .unwrap_or_else(|| state_generic_ident.to_string());
    let state_family_visit_macro_ident = state_family_visitor_macro_ident(&state_generic_name);
    let machine_callback_ident = machine_family_callback_ident(machine_info);
    let machine_visitor_macro_ident = machine_visitor_macro_ident(machine_info);
    let state_presentation_macro_ident = machine_state_presentation_entry_macro_ident(machine_info);
    let state_presentation_entry_macro =
        match generate_state_presentation_entry_macro(machine_info, &state_presentation_macro_ident) {
            Ok(tokens) => tokens,
            Err(err) => return err,
        };
    let variant_builder_init_macro_ident = machine_variant_builder_init_macro_ident(machine_info);
    let variant_builder_init_macro = generate_variant_builder_init_macro(
        machine_info,
        &parsed_machine,
        &machine_ident,
        &variant_builder_init_macro_ident,
    );
    let field_type_aliases = generate_field_type_aliases(machine_info, item);
    let callback = match generate_machine_family_callback(
        machine_info,
        &parsed_machine,
        &machine_ident,
        &state_generic_ident,
        &machine_callback_ident,
        &machine_visitor_macro_ident,
    ) {
        Ok(tokens) => tokens,
        Err(err) => return err,
    };

    quote! {
        #field_type_aliases
        #state_presentation_entry_macro
        #variant_builder_init_macro
        #callback
        #state_family_visit_macro_ident!(#machine_callback_ident);
    }
}

fn extract_state_generic_ident(generics: &Generics) -> Result<Ident, TokenStream> {
    let Some(first_param) = generics.params.first() else {
        return Err(
            syn::Error::new(
                Span::call_site(),
                "Machine struct must have a state generic as its first type parameter.",
            )
            .to_compile_error(),
        );
    };

    if let GenericParam::Type(first_type) = first_param {
        return Ok(first_type.ident.clone());
    }

    Err(
        syn::Error::new(
            Span::call_site(),
            "Machine state generic must be a type parameter.",
        )
        .to_compile_error(),
    )
}

fn generate_machine_family_callback(
    machine_info: &MachineInfo,
    parsed_machine: &ParsedMachineInfo,
    machine_ident: &Ident,
    state_generic_ident: &Ident,
    callback_ident: &Ident,
    machine_visitor_macro_ident: &Ident,
) -> Result<TokenStream, TokenStream> {
    let state_family_name = machine_info
        .state_generic_name
        .clone()
        .unwrap_or_else(|| state_generic_ident.to_string());
    let state_target_resolver_macro_ident =
        state_family_target_resolver_macro_ident(&state_family_name);
    let machine_target_resolver_macro_ident =
        machine_transition_target_resolver_macro_ident(machine_info);
    let machine_validator_contract_macro_ident =
        machine_validator_contract_macro_ident(machine_info);
    let transition_support = transition_support(machine_info);
    let transition_support_module_ident = transition_support_module_ident(machine_info);
    let struct_definition = generate_machine_struct_definition(
        parsed_machine,
        machine_ident,
        state_generic_ident,
        &transition_support_module_ident,
    );
    let builder_support = generate_builder_support(machine_info, parsed_machine, machine_ident);
    let machine_state_surface =
        generate_machine_state_surface(machine_info, parsed_machine, machine_ident)?;
    let introspection_impls = generate_machine_introspection_impls(
        machine_info,
        parsed_machine,
        machine_ident,
        state_generic_ident,
    );
    let machine_module_ident = machine_state_module_ident(machine_info);
    let machine_vis = parsed_machine.vis.clone();
    let validator_support = generate_validator_support_macro(
        machine_info,
        parsed_machine,
        machine_ident,
        &machine_module_ident,
        &machine_vis,
    );
    let extra_machine_generics = extra_generics(&parsed_machine.generics);
    let extra_generic_param_entries = extra_machine_generics
        .params
        .iter()
        .map(|param| quote! { { #param } })
        .collect::<Vec<_>>();
    let extra_generic_arg_entries = extra_machine_generics
        .params
        .iter()
        .map(|param| {
            let arg = generic_argument_tokens_for_machine_contract(param);
            quote! { { #arg } }
        })
        .collect::<Vec<_>>();
    let extra_where_predicate_entries = extra_machine_generics
        .where_clause
        .iter()
        .flat_map(|where_clause| where_clause.predicates.iter())
        .map(|predicate| quote! { { #predicate } })
        .collect::<Vec<_>>();
    let validator_field_entries = parsed_machine.fields.iter().map(|field| {
        let field_ident = &field.ident;
        let field_ty = &field.field_type;
        quote! {
            {
                name = #field_ident,
                ty = #field_ty
            }
        }
    });

    Ok(quote! {
        #[doc(hidden)]
        macro_rules! #callback_ident {
            (
                family = $family:ident,
                state_trait = $state_trait:ident,
                uninitialized = $uninitialized:ident,
                variants = [
                    $(
                        {
                            marker = $variant:ident,
                            is_fn = $is_fn:ident,
                            data = $data:ty,
                            rust_name = $rust_name:literal,
                            has_data = $has_data:tt,
                            has_presentation = $has_presentation:tt,
                            has_metadata = $has_metadata:tt,
                            presentation = {
                                label = $label:expr,
                                description = $description:expr,
                                metadata = $metadata:tt
                            }
                        }
                    ),* $(,)?
                ],
            ) => {
                #transition_support
                #[doc(hidden)]
                macro_rules! #machine_visitor_macro_ident {
                    ($callback:ident) => {
                        $callback! {
                            variants = [
                                $(
                                    {
                                        marker = $variant,
                                        has_data = $has_data
                                    }
                                ),*
                            ],
                        }
                    };
                }
                #[doc(hidden)]
                macro_rules! #machine_target_resolver_macro_ident {
                    ($callback:ident, $target:ident) => {
                        #state_target_resolver_macro_ident!($callback, $target);
                    };
                }
                #[doc(hidden)]
                macro_rules! #machine_validator_contract_macro_ident {
                    ($callback:ident) => {
                        $callback! {
                            machine = #machine_ident,
                            state_family = $family,
                            state_trait = $state_trait,
                            machine_module = #machine_module_ident,
                            machine_vis = #machine_vis,
                            extra_generics = {
                                params = [
                                    #(#extra_generic_param_entries),*
                                ],
                                args = [
                                    #(#extra_generic_arg_entries),*
                                ],
                                where_predicates = [
                                    #(#extra_where_predicate_entries),*
                                ],
                            },
                            fields = [
                                #(#validator_field_entries),*
                            ],
                            variants = [
                                $(
                                    {
                                        marker = $variant,
                                        validator = $is_fn,
                                        data = $data,
                                        has_data = $has_data
                                    }
                                ),*
                            ],
                        }
                    };
                }
                #validator_support
                #struct_definition
                #builder_support
                #machine_state_surface
                #introspection_impls
            };
        }
    })
}

fn machine_family_callback_ident(machine_info: &MachineInfo) -> Ident {
    format_ident!(
        "__statum_emit_{}_from_state_family",
        to_snake_case(&machine_info.name)
    )
}

fn machine_visitor_macro_ident(machine_info: &MachineInfo) -> Ident {
    format_ident!(
        "__statum_visit_{}_machine",
        to_snake_case(&machine_info.name)
    )
}

fn machine_transition_target_resolver_macro_ident(machine_info: &MachineInfo) -> Ident {
    format_ident!(
        "__statum_resolve_{}_transition_target",
        to_snake_case(&machine_info.name)
    )
}

fn machine_validator_contract_macro_ident(machine_info: &MachineInfo) -> Ident {
    format_ident!(
        "__statum_visit_{}_validators",
        to_snake_case(&machine_info.name)
    )
}

fn machine_validator_support_macro_ident(machine_info: &MachineInfo) -> Ident {
    format_ident!(
        "__statum_expand_{}_validators",
        to_snake_case(&machine_info.name)
    )
}

fn machine_builder_struct_ident(machine_info: &MachineInfo) -> Ident {
    format_ident!("__statum_{}_builder", to_snake_case(&machine_info.name))
}

fn machine_variant_builder_init_macro_ident(machine_info: &MachineInfo) -> Ident {
    format_ident!(
        "__statum_emit_{}_variant_builder",
        to_snake_case(&machine_info.name)
    )
}

fn machine_state_presentation_entry_macro_ident(machine_info: &MachineInfo) -> Ident {
    format_ident!(
        "__statum_emit_{}_state_presentation_entry",
        to_snake_case(&machine_info.name)
    )
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SupportedMachineDerive {
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
}

impl SupportedMachineDerive {
    fn from_path(path: &syn::Path) -> Option<Self> {
        let last_ident = path.segments.last().map(|segment| segment.ident.to_string());
        match last_ident.as_deref() {
            Some("Clone") => Some(Self::Clone),
            Some("Copy") => Some(Self::Copy),
            Some("Debug") => Some(Self::Debug),
            Some("Default") => Some(Self::Default),
            Some("Eq") => Some(Self::Eq),
            Some("Hash") => Some(Self::Hash),
            Some("Ord") => Some(Self::Ord),
            Some("PartialEq") => Some(Self::PartialEq),
            Some("PartialOrd") => Some(Self::PartialOrd),
            _ => None,
        }
    }

    fn bound_path(self) -> syn::Path {
        match self {
            Self::Clone => syn::parse_quote!(::core::clone::Clone),
            Self::Copy => syn::parse_quote!(::core::marker::Copy),
            Self::Debug => syn::parse_quote!(::core::fmt::Debug),
            Self::Default => syn::parse_quote!(::core::default::Default),
            Self::Eq => syn::parse_quote!(::core::cmp::Eq),
            Self::Hash => syn::parse_quote!(::core::hash::Hash),
            Self::Ord => syn::parse_quote!(::core::cmp::Ord),
            Self::PartialEq => syn::parse_quote!(::core::cmp::PartialEq),
            Self::PartialOrd => syn::parse_quote!(::core::cmp::PartialOrd),
        }
    }
}

fn collect_supported_machine_derives(derives: &[syn::Path]) -> Vec<SupportedMachineDerive> {
    let mut supported = Vec::new();
    for derive in derives {
        let Some(kind) = SupportedMachineDerive::from_path(derive) else {
            continue;
        };
        if !supported.contains(&kind) {
            supported.push(kind);
        }
    }
    supported
}

fn machine_struct_generics_tokens(
    parsed_machine: &ParsedMachineInfo,
    state_generic_ident: &Ident,
) -> TokenStream {
    let extra_machine_generics = extra_generics(&parsed_machine.generics);
    let extra_params = extra_machine_generics.params.iter();
    let extra_where_clause = extra_machine_generics.where_clause.clone();

    if extra_machine_generics.params.is_empty() {
        quote! {
            <#state_generic_ident: $state_trait + statum::StateMarker = $uninitialized>
        }
    } else {
        quote! {
            <#state_generic_ident: $state_trait + statum::StateMarker, #(#extra_params),*>
            #extra_where_clause
        }
    }
}

fn machine_impl_generics_tokens(
    parsed_machine: &ParsedMachineInfo,
    state_generic_ident: &Ident,
) -> TokenStream {
    let extra_machine_generics = extra_generics(&parsed_machine.generics);
    let extra_params = extra_machine_generics.params.iter();

    if extra_machine_generics.params.is_empty() {
        quote! {
            <#state_generic_ident: $state_trait + statum::StateMarker>
        }
    } else {
        quote! {
            <#state_generic_ident: $state_trait + statum::StateMarker, #(#extra_params),*>
        }
    }
}

fn machine_supported_derive_where_clause(
    parsed_machine: &ParsedMachineInfo,
    state_generic_ident: &Ident,
    bound: &syn::Path,
) -> TokenStream {
    let extra_where_clause = extra_generics(&parsed_machine.generics).where_clause.clone();
    let mut predicates = vec![quote! {
        <#state_generic_ident as statum::StateMarker>::Data: #bound
    }];
    predicates.extend(parsed_machine.fields.iter().map(|field| {
        let field_ty = &field.field_type;
        quote! { #field_ty: #bound }
    }));
    with_appended_where_clause(
        extra_where_clause.as_ref(),
        quote! { #(#predicates),* },
    )
}

fn generate_machine_supported_derive_impls(
    parsed_machine: &ParsedMachineInfo,
    machine_ident: &Ident,
    state_generic_ident: &Ident,
) -> TokenStream {
    let supported_derives = collect_supported_machine_derives(&parsed_machine.derives);
    if supported_derives.is_empty() {
        return quote! {};
    }

    let impl_generics = machine_impl_generics_tokens(parsed_machine, state_generic_ident);
    let self_ty = machine_type_with_state(
        quote! { #machine_ident },
        &parsed_machine.generics,
        quote! { #state_generic_ident },
    );
    let field_idents = parsed_machine
        .fields
        .iter()
        .map(|field| field.ident.clone())
        .collect::<Vec<_>>();
    let field_names = field_idents
        .iter()
        .map(|field_ident| LitStr::new(&field_ident.to_string(), Span::call_site()))
        .collect::<Vec<_>>();
    let clone_fields = field_idents
        .iter()
        .map(|field_ident| {
            quote! {
                #field_ident: ::core::clone::Clone::clone(&self.#field_ident)
            }
        })
        .collect::<Vec<_>>();
    let default_fields = field_idents
        .iter()
        .map(|field_ident| {
            quote! {
                #field_ident: ::core::default::Default::default()
            }
        })
        .collect::<Vec<_>>();
    let partial_eq_checks = std::iter::once(quote! { self.state_data == other.state_data })
        .chain(
            field_idents
                .iter()
                .map(|field_ident| quote! { self.#field_ident == other.#field_ident }),
        )
        .collect::<Vec<_>>();
    let hash_calls = std::iter::once(quote! {
        ::core::hash::Hash::hash(&self.state_data, state);
    })
    .chain(field_idents.iter().map(|field_ident| {
        quote! {
            ::core::hash::Hash::hash(&self.#field_ident, state);
        }
    }))
    .collect::<Vec<_>>();
    let left_order_members = std::iter::once(quote! { &self.state_data })
        .chain(field_idents.iter().map(|field_ident| quote! { &self.#field_ident }))
        .collect::<Vec<_>>();
    let right_order_members = std::iter::once(quote! { &other.state_data })
        .chain(field_idents.iter().map(|field_ident| quote! { &other.#field_ident }))
        .collect::<Vec<_>>();
    let debug_fields = field_names
        .iter()
        .zip(field_idents.iter())
        .map(|(field_name, field_ident)| quote! { .field(#field_name, &self.#field_ident) })
        .collect::<Vec<_>>();

    let mut impls = Vec::with_capacity(supported_derives.len());
    for derive in supported_derives {
        let bound = derive.bound_path();
        let where_clause =
            machine_supported_derive_where_clause(parsed_machine, state_generic_ident, &bound);
        let tokens = match derive {
            SupportedMachineDerive::Clone => quote! {
                impl #impl_generics ::core::clone::Clone for #self_ty #where_clause {
                    fn clone(&self) -> Self {
                        Self {
                            marker: core::marker::PhantomData,
                            state_data: ::core::clone::Clone::clone(&self.state_data),
                            #(#clone_fields),*
                        }
                    }
                }
            },
            SupportedMachineDerive::Copy => quote! {
                impl #impl_generics ::core::marker::Copy for #self_ty #where_clause {}
            },
            SupportedMachineDerive::Debug => quote! {
                impl #impl_generics ::core::fmt::Debug for #self_ty #where_clause {
                    fn fmt(
                        &self,
                        formatter: &mut ::core::fmt::Formatter<'_>,
                    ) -> ::core::result::Result<(), ::core::fmt::Error> {
                        formatter
                            .debug_struct(stringify!(#machine_ident))
                            .field("marker", &self.marker)
                            .field("state_data", &self.state_data)
                            #(#debug_fields)*
                            .finish()
                    }
                }
            },
            SupportedMachineDerive::Default => quote! {
                impl #impl_generics ::core::default::Default for #self_ty #where_clause {
                    fn default() -> Self {
                        Self {
                            marker: core::marker::PhantomData,
                            state_data: ::core::default::Default::default(),
                            #(#default_fields),*
                        }
                    }
                }
            },
            SupportedMachineDerive::Eq => quote! {
                impl #impl_generics ::core::cmp::Eq for #self_ty #where_clause {}
            },
            SupportedMachineDerive::Hash => quote! {
                impl #impl_generics ::core::hash::Hash for #self_ty #where_clause {
                    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                        #(#hash_calls)*
                    }
                }
            },
            SupportedMachineDerive::Ord => quote! {
                impl #impl_generics ::core::cmp::Ord for #self_ty #where_clause {
                    fn cmp(&self, other: &Self) -> ::core::cmp::Ordering {
                        let left = (#(#left_order_members,)*);
                        let right = (#(#right_order_members,)*);
                        ::core::cmp::Ord::cmp(&left, &right)
                    }
                }
            },
            SupportedMachineDerive::PartialEq => quote! {
                impl #impl_generics ::core::cmp::PartialEq for #self_ty #where_clause {
                    fn eq(&self, other: &Self) -> bool {
                        #(#partial_eq_checks)&&*
                    }
                }
            },
            SupportedMachineDerive::PartialOrd => quote! {
                impl #impl_generics ::core::cmp::PartialOrd for #self_ty #where_clause {
                    fn partial_cmp(&self, other: &Self) -> ::core::option::Option<::core::cmp::Ordering> {
                        let left = (#(#left_order_members,)*);
                        let right = (#(#right_order_members,)*);
                        ::core::cmp::PartialOrd::partial_cmp(&left, &right)
                    }
                }
            },
        };
        impls.push(tokens);
    }

    quote! {
        #(#impls)*
    }
}

fn generate_machine_struct_definition(
    parsed_machine: &ParsedMachineInfo,
    machine_ident: &Ident,
    state_generic_ident: &Ident,
    transition_support_module_ident: &Ident,
) -> TokenStream {
    let mut field_tokens = Vec::with_capacity(parsed_machine.fields.len());
    for field in &parsed_machine.fields {
        let field_ident = &field.ident;
        let field_vis = &field.vis;
        let field_ty = &field.field_type;
        field_tokens.push(quote! { #field_vis #field_ident: #field_ty });
    }

    let passthrough_derives = parsed_machine
        .derives
        .iter()
        .filter(|derive| SupportedMachineDerive::from_path(derive).is_none())
        .cloned()
        .collect::<Vec<_>>();
    let derives = if passthrough_derives.is_empty() {
        quote! {}
    } else {
        let derive_tokens = passthrough_derives;
        quote! {
            #[derive(#(#derive_tokens),*)]
        }
    };

    let vis = parsed_machine.vis.clone();
    let struct_generics = machine_struct_generics_tokens(parsed_machine, state_generic_ident);
    let impl_generics = machine_impl_generics_tokens(parsed_machine, state_generic_ident);
    let extra_machine_generics = extra_generics(&parsed_machine.generics);
    let extra_where_clause = extra_machine_generics.where_clause.clone();
    let manual_derive_impls =
        generate_machine_supported_derive_impls(parsed_machine, machine_ident, state_generic_ident);
    let transition_trait_impl_generics = if extra_machine_generics.params.is_empty() {
        quote! {
            <#state_generic_ident: $state_trait + statum::StateMarker, N>
        }
    } else {
        let extra_params = extra_machine_generics.params.iter();
        quote! {
            <#state_generic_ident: $state_trait + statum::StateMarker, N, #(#extra_params),*>
        }
    };
    let transition_with_data_impl_generics = if extra_machine_generics.params.is_empty() {
        quote! {
            <#state_generic_ident: $state_trait + statum::StateMarker, Data>
        }
    } else {
        let extra_params = extra_machine_generics.params.iter();
        quote! {
            <#state_generic_ident: $state_trait + statum::StateMarker, Data, #(#extra_params),*>
        }
    };
    let transition_to_impl_where_clause = with_appended_where_clause(
        extra_machine_generics.where_clause.as_ref(),
        quote! {
            N: $state_trait + statum::UnitState,
            Self: #transition_support_module_ident::EdgeTo<N>
        },
    );
    let transition_map_impl_where_clause = with_appended_where_clause(
        extra_machine_generics.where_clause.as_ref(),
        quote! {
            N: $state_trait + statum::DataState,
            Self: #transition_support_module_ident::EdgeTo<N>
        },
    );
    let transition_with_data_impl_where_clause = with_appended_where_clause(
        extra_machine_generics.where_clause.as_ref(),
        quote! {
            Self: #transition_support_module_ident::TransitionWithBinding<Data>,
            <Self as #transition_support_module_ident::TransitionWithBinding<Data>>::NextState:
                $state_trait + statum::DataState
        },
    );
    let self_ty = machine_type_with_state(
        quote! { #machine_ident },
        &parsed_machine.generics,
        quote! { #state_generic_ident },
    );
    let field_idents = parsed_machine
        .fields
        .iter()
        .map(|field| field.ident.clone())
        .collect::<Vec<_>>();
    let next_machine_ty = machine_type_with_state(
        quote! { #machine_ident },
        &parsed_machine.generics,
        quote! { N },
    );
    let bound_next_state_ty =
        quote! { <Self as #transition_support_module_ident::TransitionWithBinding<Data>>::NextState };
    let bound_next_machine_ty = machine_type_with_state(
        quote! { #machine_ident },
        &parsed_machine.generics,
        bound_next_state_ty.clone(),
    );
    let transition_with_body = if field_idents.is_empty() {
        quote! {
            let Self {
                marker: _,
                state_data: _,
            } = self;

            #machine_ident {
                marker: core::marker::PhantomData,
                state_data: __statum_transition_data,
            }
        }
    } else {
        quote! {
            let Self {
                marker: _,
                state_data: _,
                #(#field_idents),*
            } = self;

            #machine_ident {
                marker: core::marker::PhantomData,
                state_data: __statum_transition_data,
                #(#field_idents),*
            }
        }
    };
    let transition_map_body = if field_idents.is_empty() {
        quote! {
            let Self {
                marker: _,
                state_data,
            } = self;

            #machine_ident {
                marker: core::marker::PhantomData,
                state_data: f(state_data),
            }
        }
    } else {
        quote! {
            let Self {
                marker: _,
                state_data,
                #(#field_idents),*
            } = self;

            #machine_ident {
                marker: core::marker::PhantomData,
                state_data: f(state_data),
                #(#field_idents),*
            }
        }
    };

    quote! {
        #derives
        #[allow(dead_code)]
        #vis struct #machine_ident #struct_generics {
            marker: core::marker::PhantomData<#state_generic_ident>,
            pub state_data: <#state_generic_ident as statum::StateMarker>::Data,
            #( #field_tokens ),*
        }

        #manual_derive_impls

        #[allow(dead_code)]
        impl #impl_generics #self_ty #extra_where_clause {
            #vis fn transition_map<N, F>(self, f: F) -> #next_machine_ty
            where
                N: $state_trait + statum::DataState,
                Self: #transition_support_module_ident::EdgeTo<N>
                    + statum::CanTransitionMap<N, Output = #next_machine_ty>,
                F: FnOnce(<Self as statum::CanTransitionMap<N>>::CurrentData) ->
                    <N as statum::StateMarker>::Data,
            {
                <Self as statum::CanTransitionMap<N>>::transition_map(self, f)
            }

            #[doc(hidden)]
            fn __statum_transition_with_state<N>(
                self,
                __statum_transition_data: <N as statum::StateMarker>::Data,
            ) -> #next_machine_ty
            where
                N: $state_trait + statum::StateMarker,
            {
                #transition_with_body
            }

            #[doc(hidden)]
            fn __statum_transition_map_state<N, F>(self, f: F) -> #next_machine_ty
            where
                N: $state_trait + statum::StateMarker,
                F: FnOnce(<#state_generic_ident as statum::StateMarker>::Data) ->
                    <N as statum::StateMarker>::Data,
            {
                #transition_map_body
            }
        }

        #[allow(dead_code)]
        impl #transition_trait_impl_generics #transition_support_module_ident::TransitionTo<N> for #self_ty
        #transition_to_impl_where_clause
        {
            type Output = #next_machine_ty;

            fn transition(self) -> Self::Output {
                self.__statum_transition_with_state::<N>(())
            }
        }

        #[allow(dead_code)]
        impl #transition_with_data_impl_generics #transition_support_module_ident::TransitionWith<Data>
            for #self_ty
        #transition_with_data_impl_where_clause
        {
            type Output = #bound_next_machine_ty;

            fn transition_with(self, __statum_transition_data: Data) -> Self::Output {
                self.__statum_transition_with_state::<
                    <Self as #transition_support_module_ident::TransitionWithBinding<Data>>::NextState,
                >(__statum_transition_data)
            }
        }

        #[allow(dead_code)]
        impl #transition_trait_impl_generics statum::CanTransitionTo<N> for #self_ty
        #transition_to_impl_where_clause
        {
            type Output = <Self as #transition_support_module_ident::TransitionTo<N>>::Output;

            fn transition_to(self) -> Self::Output {
                <Self as #transition_support_module_ident::TransitionTo<N>>::transition(self)
            }
        }

        #[allow(dead_code)]
        impl #transition_with_data_impl_generics statum::CanTransitionWith<Data> for #self_ty
        #transition_with_data_impl_where_clause
        {
            type NextState =
                <Self as #transition_support_module_ident::TransitionWithBinding<Data>>::NextState;
            type Output = <Self as #transition_support_module_ident::TransitionWith<Data>>::Output;

            fn transition_with_data(self, __statum_transition_data: Data) -> Self::Output {
                <Self as #transition_support_module_ident::TransitionWith<Data>>::transition_with(
                    self,
                    __statum_transition_data,
                )
            }
        }

        #[allow(dead_code)]
        impl #transition_trait_impl_generics statum::CanTransitionMap<N> for #self_ty
        #transition_map_impl_where_clause
        {
            type CurrentData = <#state_generic_ident as statum::StateMarker>::Data;
            type Output = #next_machine_ty;

            fn transition_map<F>(self, f: F) -> Self::Output
            where
                F: FnOnce(Self::CurrentData) -> <N as statum::StateMarker>::Data,
            {
                self.__statum_transition_map_state::<N, F>(f)
            }
        }
    }
}

fn transition_support(machine_info: &MachineInfo) -> TokenStream {
    let support_module_ident = transition_support_module_ident(machine_info);

    quote! {
        #[allow(dead_code)]
        #[doc(hidden)]
        mod #support_module_ident {
            use super::*;

            pub trait EdgeTo<N: $state_trait> {}

            pub trait TransitionTo<N: $state_trait + statum::UnitState> {
                type Output;

                fn transition(self) -> Self::Output;
            }

            pub trait TransitionWithBinding<Data> {
                type NextState:
                    $state_trait
                    + statum::DataState
                    + statum::StateMarker<Data = Data>;
            }

            pub trait TransitionWith<Data>: TransitionWithBinding<Data> {
                type Output;

                fn transition_with(self, data: Data) -> Self::Output;
            }
        }

        #[allow(unused_imports)]
        use #support_module_ident::{TransitionTo as _, TransitionWith as _};
    }
}

fn generate_builder_support(
    machine_info: &MachineInfo,
    parsed_machine: &ParsedMachineInfo,
    machine_ident: &Ident,
) -> TokenStream {
    let builder_ident = machine_builder_struct_ident(machine_info);
    let builder_init_macro_ident = machine_variant_builder_init_macro_ident(machine_info);
    let builder_vis = parsed_machine.vis.clone();
    let extra_machine_generics = extra_generics(&parsed_machine.generics);
    let parsed_fields = parsed_machine.field_idents_and_types();
    let field_names = parsed_fields
        .iter()
        .map(|(field_ident, _)| field_ident.clone())
        .collect::<Vec<_>>();
    let field_types = parsed_fields
        .iter()
        .map(|(_, field_ty)| field_ty.clone())
        .collect::<Vec<_>>();
    let slot_state_idents = (0..field_names.len() + 1)
        .map(|idx| format_ident!("__STATUM_SLOT_{}_SET", idx))
        .collect::<Vec<_>>();

    let mut builder_defaults = builder_generics(&extra_machine_generics, false, &slot_state_idents, true);
    builder_defaults
        .params
        .insert(0, syn::parse_quote!(__StatumState));
    if builder_defaults.lt_token.is_none() {
        builder_defaults.lt_token = Some(Default::default());
        builder_defaults.gt_token = Some(Default::default());
    }
    builder_defaults.where_clause = None;
    let builder_struct_where_clause = with_appended_where_clause(
        extra_machine_generics.where_clause.as_ref(),
        quote! { __StatumState: $state_trait },
    );

    let mut builder_impl_generics_decl =
        builder_generics(&extra_machine_generics, false, &slot_state_idents, false);
    builder_impl_generics_decl
        .params
        .insert(0, syn::parse_quote!(__StatumState));
    if builder_impl_generics_decl.lt_token.is_none() {
        builder_impl_generics_decl.lt_token = Some(Default::default());
        builder_impl_generics_decl.gt_token = Some(Default::default());
    }
    builder_impl_generics_decl.where_clause = None;
    let (builder_impl_generics, builder_ty_generics, _builder_where_clause) =
        builder_impl_generics_decl.split_for_impl();
    let builder_where_clause = with_appended_where_clause(
        extra_machine_generics.where_clause.as_ref(),
        quote! { __StatumState: $state_trait },
    );

    let mut build_impl_generics_decl = extra_machine_generics.clone();
    build_impl_generics_decl
        .params
        .insert(0, syn::parse_quote!(__StatumState));
    if build_impl_generics_decl.lt_token.is_none() {
        build_impl_generics_decl.lt_token = Some(Default::default());
        build_impl_generics_decl.gt_token = Some(Default::default());
    }
    build_impl_generics_decl.where_clause = None;
    let (build_impl_generics, _, _) = build_impl_generics_decl.split_for_impl();

    let builder_struct_fields = field_names
        .iter()
        .zip(field_types.iter())
        .map(|(field_name, field_ty)| {
            quote! { #field_name: core::option::Option<#field_ty> }
        })
        .collect::<Vec<_>>();
    let state_data_target_ty_generics = {
        let mut slot_values = vec![quote! { true }];
        slot_values.extend(
            slot_state_idents
                .iter()
                .skip(1)
                .map(|slot_state_ident| quote! { #slot_state_ident }),
        );
        builder_type_arguments_tokens(quote! { __StatumState }, &extra_machine_generics, &slot_values)
    };
    let state_data_assignments = std::iter::once(
        quote! { __statum_state_data: core::option::Option::Some(value) },
    )
    .chain(field_names.iter().map(|field_name| {
        quote! { #field_name: self.#field_name }
    }))
    .collect::<Vec<_>>();
    let field_setters = field_names.iter().enumerate().map(|(field_idx, field_name)| {
        let field_ty = &field_types[field_idx];
        let target_ty_generics = {
            let slot_values = slot_state_idents
                .iter()
                .enumerate()
                .map(|(slot_idx, slot_state_ident)| {
                    if slot_idx == field_idx + 1 {
                        quote! { true }
                    } else {
                        quote! { #slot_state_ident }
                    }
                })
                .collect::<Vec<_>>();
            builder_type_arguments_tokens(
                quote! { __StatumState },
                &extra_machine_generics,
                &slot_values,
            )
        };
        let field_assignments = std::iter::once(
            quote! { __statum_state_data: self.__statum_state_data },
        )
        .chain(field_names.iter().enumerate().map(|(idx, name)| {
            if idx == field_idx {
                quote! { #name: core::option::Option::Some(value) }
            } else {
                quote! { #name: self.#name }
            }
        }))
        .collect::<Vec<_>>();

        quote! {
            #builder_vis fn #field_name(self, value: #field_ty) -> #builder_ident #target_ty_generics {
                #builder_ident {
                    #(#field_assignments),*
                }
            }
        }
    });
    let state_machine_ty = machine_type_with_state(
        quote! { #machine_ident },
        &parsed_machine.generics,
        quote! { __StatumState },
    );
    let complete_builder_ty_generics = {
        let slot_values = slot_state_idents
            .iter()
            .map(|_| quote! { true })
            .collect::<Vec<_>>();
        builder_type_arguments_tokens(
            quote! { __StatumState },
            &extra_machine_generics,
            &slot_values,
        )
    };
    let build_where_clause = with_appended_where_clause(
        extra_machine_generics.where_clause.as_ref(),
        quote! { __StatumState: $state_trait },
    );
    let field_bindings = field_names.iter().map(|field_name| {
        let message = format!("statum internal error: `{field_name}` was not set before build");
        quote! {
            let #field_name = self.#field_name.expect(#message);
        }
    });
    let machine_initialization = if field_names.is_empty() {
        quote! {
            #machine_ident {
                marker: core::marker::PhantomData,
                state_data,
            }
        }
    } else {
        quote! {
            #machine_ident {
                marker: core::marker::PhantomData,
                state_data,
                #(#field_names,)*
            }
        }
    };
    quote! {
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        #[allow(dead_code)]
        #builder_vis struct #builder_ident #builder_defaults #builder_struct_where_clause {
            __statum_state_data: core::option::Option<<__StatumState as statum::StateMarker>::Data>,
            #(#builder_struct_fields),*
        }

        $( #builder_init_macro_ident!(variant = $variant, has_data = $has_data); )*

        #[allow(dead_code)]
        impl #builder_impl_generics #builder_ident #builder_ty_generics #builder_where_clause {
            #builder_vis fn state_data(self, value: <__StatumState as statum::StateMarker>::Data) -> #builder_ident #state_data_target_ty_generics {
                #builder_ident {
                    #(#state_data_assignments),*
                }
            }

            #(#field_setters)*
        }

        #[allow(dead_code)]
        impl #build_impl_generics #builder_ident #complete_builder_ty_generics #build_where_clause {
            #builder_vis fn build(self) -> #state_machine_ty {
                let state_data = self.__statum_state_data.expect(
                    "statum internal error: `state_data` was not set before build",
                );
                #(#field_bindings)*
                #machine_initialization
            }
        }
    }
}

fn generate_variant_builder_init_macro(
    machine_info: &MachineInfo,
    parsed_machine: &ParsedMachineInfo,
    machine_ident: &Ident,
    macro_ident: &Ident,
) -> TokenStream {
    let builder_ident = machine_builder_struct_ident(machine_info);
    let builder_vis = parsed_machine.vis.clone();
    let extra_machine_generics = extra_generics(&parsed_machine.generics);
    let extra_where_clause = extra_machine_generics.where_clause.clone();
    let extra_impl_generics = if extra_machine_generics.params.is_empty() {
        quote! {}
    } else {
        let extra_params = extra_machine_generics.params.iter();
        quote! { <#(#extra_params),*> }
    };
    let field_names = parsed_machine
        .field_idents_and_types()
        .into_iter()
        .map(|(field_ident, _)| field_ident)
        .collect::<Vec<_>>();
    let builder_field_defaults = field_names
        .iter()
        .map(|field_name| {
            quote! { #field_name: core::option::Option::None }
        })
        .collect::<Vec<_>>();
    let unit_builder_ty_generics = {
        let mut slot_values = vec![quote! { true }];
        slot_values.extend((0..field_names.len()).map(|_| quote! { false }));
        builder_type_arguments_tokens(quote! { $variant }, &extra_machine_generics, &slot_values)
    };
    let data_builder_ty_generics = {
        let mut slot_values = vec![quote! { false }];
        slot_values.extend((0..field_names.len()).map(|_| quote! { false }));
        builder_type_arguments_tokens(quote! { $variant }, &extra_machine_generics, &slot_values)
    };
    let variant_machine_ty = machine_type_with_state(
        quote! { #machine_ident },
        &parsed_machine.generics,
        quote! { $variant },
    );

    quote! {
        #[doc(hidden)]
        macro_rules! #macro_ident {
            (variant = $variant:ident, has_data = false) => {
                #[allow(dead_code)]
                impl #extra_impl_generics #variant_machine_ty #extra_where_clause {
                    #builder_vis fn builder() -> #builder_ident #unit_builder_ty_generics {
                        #builder_ident {
                            __statum_state_data: core::option::Option::Some(()),
                            #(#builder_field_defaults),*
                        }
                    }
                }
            };
            (variant = $variant:ident, has_data = true) => {
                #[allow(dead_code)]
                impl #extra_impl_generics #variant_machine_ty #extra_where_clause {
                    #builder_vis fn builder() -> #builder_ident #data_builder_ty_generics {
                        #builder_ident {
                            __statum_state_data: core::option::Option::None,
                            #(#builder_field_defaults),*
                        }
                    }
                }
            };
        }
    }
}

fn builder_type_arguments_tokens(
    state_ty: TokenStream,
    extra_machine_generics: &Generics,
    slot_values: &[TokenStream],
) -> TokenStream {
    let mut args = vec![state_ty];
    args.extend(extra_machine_generics.params.iter().map(|param| match param {
        GenericParam::Lifetime(lifetime) => {
            let lifetime = &lifetime.lifetime;
            quote! { #lifetime }
        }
        GenericParam::Type(ty) => {
            let ident = &ty.ident;
            quote! { #ident }
        }
        GenericParam::Const(const_param) => {
            let ident = &const_param.ident;
            quote! { #ident }
        }
    }));
    args.extend(slot_values.iter().cloned());

    quote! { <#(#args),*> }
}

fn generic_argument_tokens_for_machine_contract(param: &GenericParam) -> TokenStream {
    match param {
        GenericParam::Lifetime(lifetime) => {
            let lifetime = &lifetime.lifetime;
            quote! { #lifetime }
        }
        GenericParam::Type(ty) => {
            let ident = &ty.ident;
            quote! { #ident }
        }
        GenericParam::Const(const_param) => {
            let ident = &const_param.ident;
            quote! { #ident }
        }
    }
}

fn generate_validator_support_macro(
    machine_info: &MachineInfo,
    parsed_machine: &ParsedMachineInfo,
    machine_ident: &Ident,
    machine_module_ident: &Ident,
    machine_vis: &syn::Visibility,
) -> TokenStream {
    let support_macro_ident = machine_validator_support_macro_ident(machine_info);
    let persisted_ty = quote! { $persisted };
    let machine_extra_ty_args = extra_type_arguments_tokens(&parsed_machine.generics);
    let machine_state_ty = quote! { #machine_module_ident::SomeState #machine_extra_ty_args };
    let field_pairs = parsed_machine.field_idents_and_types();
    let field_names = field_pairs
        .iter()
        .map(|(field_name, _)| field_name.clone())
        .collect::<Vec<_>>();
    let field_types = field_pairs
        .iter()
        .map(|(_, field_ty)| field_ty.clone())
        .collect::<Vec<_>>();
    let single_builder_support = generate_validator_single_builder_support(
        &persisted_ty,
        machine_ident,
        machine_module_ident,
        machine_vis,
        parsed_machine,
        &machine_state_ty,
        &field_names,
        &field_types,
        false,
    );
    let single_builder_support_async = generate_validator_single_builder_support(
        &persisted_ty,
        machine_ident,
        machine_module_ident,
        machine_vis,
        parsed_machine,
        &machine_state_ty,
        &field_names,
        &field_types,
        true,
    );
    let batch_builder_support = generate_validator_batch_builder_support(
        &persisted_ty,
        machine_ident,
        machine_module_ident,
        machine_vis,
        parsed_machine,
        &machine_state_ty,
        &field_names,
        &field_types,
        false,
    );
    let batch_builder_support_async = generate_validator_batch_builder_support(
        &persisted_ty,
        machine_ident,
        machine_module_ident,
        machine_vis,
        parsed_machine,
        &machine_state_ty,
        &field_names,
        &field_types,
        true,
    );
    let extra_machine_generics = extra_generics(&parsed_machine.generics);
    let extra_generic_param_entries = extra_machine_generics
        .params
        .iter()
        .map(|param| quote! { { #param } })
        .collect::<Vec<_>>();
    let extra_where_predicate_entries = extra_machine_generics
        .where_clause
        .iter()
        .flat_map(|where_clause| where_clause.predicates.iter())
        .map(|predicate| quote! { { #predicate } })
        .collect::<Vec<_>>();
    let (into_machine_method_generics, _, into_machine_method_where_clause) =
        extra_machine_generics.split_for_impl();
    let into_machine_builder_ident = format_ident!("__Statum{}IntoMachine", machine_ident);
    let into_machine_slot_defaults = (0..field_names.len())
        .map(|_| quote! { false })
        .collect::<Vec<_>>();
    let into_machine_builder_ty_generics = generic_argument_tokens(
        extra_machine_generics.params.iter(),
        Some(quote! { '_ }),
        &into_machine_slot_defaults,
    );
    let builder_init = field_names
        .iter()
        .map(|field_name| quote! { #field_name: core::option::Option::None })
        .collect::<Vec<_>>();

    quote! {
        #[doc(hidden)]
        macro_rules! #support_macro_ident {
            (
                persisted = $persisted:ty,
                build_variant = $build_variant_macro:ident,
                report_variant = $report_variant_macro:ident,
                validator_count = $validator_count:expr,
                async = false,
                validator_methods = $validator_methods:tt,
            ) => {
                statum::__statum_emit_validator_methods_impl! {
                    persisted = $persisted,
                    extra_generics = {
                        params = [
                            #(#extra_generic_param_entries),*
                        ],
                        where_predicates = [
                            #(#extra_where_predicate_entries),*
                        ],
                    },
                    fields = [
                        #(
                            {
                                name = #field_names,
                                ty = #field_types
                            }
                        ),*
                    ],
                    validator_methods = $validator_methods,
                }

                #[allow(unused_imports)]
                use #machine_module_ident::IntoMachinesExt as _;

                #[allow(clippy::wrong_self_convention)]
                impl $persisted {
                    #machine_vis fn into_machine #into_machine_method_generics (&self) -> #into_machine_builder_ident #into_machine_builder_ty_generics #into_machine_method_where_clause {
                        #into_machine_builder_ident {
                            __statum_item: self,
                            #(#builder_init),*
                        }
                    }
                }

                #single_builder_support
                #batch_builder_support
            };
            (
                persisted = $persisted:ty,
                build_variant = $build_variant_macro:ident,
                report_variant = $report_variant_macro:ident,
                validator_count = $validator_count:expr,
                async = true,
                validator_methods = $validator_methods:tt,
            ) => {
                statum::__statum_emit_validator_methods_impl! {
                    persisted = $persisted,
                    extra_generics = {
                        params = [
                            #(#extra_generic_param_entries),*
                        ],
                        where_predicates = [
                            #(#extra_where_predicate_entries),*
                        ],
                    },
                    fields = [
                        #(
                            {
                                name = #field_names,
                                ty = #field_types
                            }
                        ),*
                    ],
                    validator_methods = $validator_methods,
                }

                #[allow(unused_imports)]
                use #machine_module_ident::IntoMachinesExt as _;

                #[allow(clippy::wrong_self_convention)]
                impl $persisted {
                    #machine_vis fn into_machine #into_machine_method_generics (&self) -> #into_machine_builder_ident #into_machine_builder_ty_generics #into_machine_method_where_clause {
                        #into_machine_builder_ident {
                            __statum_item: self,
                            #(#builder_init),*
                        }
                    }
                }

                #single_builder_support_async
                #batch_builder_support_async
            };
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_validator_single_builder_support(
    persisted_ty: &TokenStream,
    machine_ident: &Ident,
    machine_module_ident: &Ident,
    machine_vis: &syn::Visibility,
    parsed_machine: &ParsedMachineInfo,
    machine_state_ty: &TokenStream,
    field_names: &[Ident],
    field_types: &[syn::Type],
    async_mode: bool,
) -> TokenStream {
    let builder_ident = format_ident!("__Statum{}IntoMachine", machine_ident);
    let extra_machine_generics = extra_generics(&parsed_machine.generics);
    let slot_state_idents = (0..field_names.len())
        .map(|idx| format_ident!("__STATUM_SLOT_{}_SET", idx))
        .collect::<Vec<_>>();
    let builder_defaults = builder_generics(&extra_machine_generics, true, &slot_state_idents, true);
    let builder_impl_generics_decl =
        builder_generics(&extra_machine_generics, true, &slot_state_idents, false);
    let (builder_impl_generics, builder_ty_generics, builder_where_clause) =
        builder_impl_generics_decl.split_for_impl();
    let complete_slots = slot_state_idents
        .iter()
        .map(|_| quote! { true })
        .collect::<Vec<_>>();
    let complete_builder_ty_generics = generic_argument_tokens(
        extra_machine_generics.params.iter(),
        Some(quote! { '__statum_row }),
        &complete_slots,
    );
    let complete_builder_impl_generics_decl =
        builder_generics(&extra_machine_generics, true, &[], false);
    let (complete_builder_impl_generics, _, complete_builder_where_clause) =
        complete_builder_impl_generics_decl.split_for_impl();

    let struct_fields = field_names
        .iter()
        .zip(field_types.iter())
        .map(|(field_name, field_type)| {
            quote! { #field_name: core::option::Option<#field_type> }
        })
        .collect::<Vec<_>>();
    let field_bindings = field_names
        .iter()
        .map(|field_name| {
            let message = format!("statum internal error: `{field_name}` was not set before build");
            quote! {
                let #field_name = self.#field_name.expect(#message);
            }
        })
        .collect::<Vec<_>>();
    let setters = field_names
        .iter()
        .zip(field_types.iter())
        .enumerate()
        .map(|(slot_idx, (field_name, field_type))| {
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
            let target_generics = generic_argument_tokens(
                extra_machine_generics.params.iter(),
                Some(quote! { '__statum_row }),
                &generics,
            );
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
                        __statum_item: self.__statum_item,
                        #(#assignments),*
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    let field_entries = field_names
        .iter()
        .zip(field_types.iter())
        .map(|(field_name, field_type)| {
            quote! {
                {
                    name = #field_name,
                    ty = #field_type
                }
            }
        })
        .collect::<Vec<_>>();
    let variant_builder_ty = machine_type_with_state(
        quote! { #machine_ident },
        &parsed_machine.generics,
        quote! { $variant },
    );
    let variant_state_path = quote! { #machine_module_ident::SomeState::$variant };
    let async_token = if async_mode {
        quote! { async }
    } else {
        quote! {}
    };
    let report_result = quote! {
        let mut __statum_attempts = ::std::vec::Vec::with_capacity($validator_count);
        #(#field_bindings)*
            $(
                $report_variant_macro!(
                    persisted = __statum_persisted,
                    attempts = __statum_attempts,
                    machine = #machine_ident,
                    state_family = $family,
                    machine_module = #machine_module_ident,
                    machine_builder = #variant_builder_ty,
                variant = $variant,
                state_variant = #variant_state_path,
                validator = $is_fn,
                data = $data,
                has_data = $has_data,
                fields = [#(#field_entries),*],
            );
        )*
        statum::RebuildReport {
            attempts: __statum_attempts,
            result: Err(statum::Error::InvalidState),
        }
    };

    quote! {
        #[doc(hidden)]
        #[allow(dead_code, private_bounds, private_interfaces)]
        #machine_vis struct #builder_ident #builder_defaults {
            __statum_item: &'__statum_row #persisted_ty,
            #(#struct_fields),*
        }

        #[allow(dead_code, private_bounds, private_interfaces)]
        impl #builder_impl_generics #builder_ident #builder_ty_generics #builder_where_clause {
            #(#setters)*
        }

        #[allow(dead_code, private_bounds, private_interfaces)]
        impl #complete_builder_impl_generics #builder_ident #complete_builder_ty_generics #complete_builder_where_clause {
            #machine_vis #async_token fn build(self) -> core::result::Result<#machine_state_ty, statum::Error> {
                let __statum_persisted = self.__statum_item;
                #(#field_bindings)*
                $(
                    $build_variant_macro!(
                        persisted = __statum_persisted,
                        machine = #machine_ident,
                        state_family = $family,
                        machine_module = #machine_module_ident,
                        machine_builder = #variant_builder_ty,
                        variant = $variant,
                        state_variant = #variant_state_path,
                        validator = $is_fn,
                        data = $data,
                        has_data = $has_data,
                        fields = [#(#field_entries),*],
                    );
                )*

                Err(statum::Error::InvalidState)
            }

            #machine_vis #async_token fn build_report(self) -> statum::RebuildReport<#machine_state_ty> {
                let __statum_persisted = self.__statum_item;
                #report_result
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn generate_validator_batch_builder_support(
    persisted_ty: &TokenStream,
    machine_ident: &Ident,
    machine_module_ident: &Ident,
    machine_vis: &syn::Visibility,
    parsed_machine: &ParsedMachineInfo,
    machine_state_ty: &TokenStream,
    field_names: &[Ident],
    field_types: &[syn::Type],
    async_mode: bool,
) -> TokenStream {
    let builder_ident = format_ident!("__Statum{}IntoMachines", machine_ident);
    let by_builder_ident = format_ident!("__Statum{}IntoMachinesBy", machine_ident);
    let machine_generics = &parsed_machine.generics;
    let extra_machine_generics = extra_generics(machine_generics);
    let extra_machine_ty_args = extra_type_arguments_tokens(machine_generics);
    let fields_ty = quote! { #machine_module_ident::Fields #extra_machine_ty_args };
    let extra_impl_params = extra_machine_generics
        .params
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let into_machines_trait_args = validator_support_prefixed_generics_argument_tokens(
        quote! { #persisted_ty },
        extra_machine_generics.params.iter(),
    );
    let into_machines_impl_generics = if extra_impl_params.is_empty() {
        quote! { <T> }
    } else {
        quote! { <T, #(#extra_impl_params),*> }
    };
    let into_machines_where_clause = validator_support_merged_where_clause_tokens(
        extra_machine_generics.where_clause.as_ref(),
        vec![quote! { T: Into<Vec<#persisted_ty>> }],
    );

    let field_builder_chain = quote! { #(.#field_names(#field_names.clone()))* };
    let per_item_builder_chain = quote! { #(.#field_names(__statum_fields.#field_names))* };
    let await_token = if async_mode {
        quote! { .await }
    } else {
        quote! {}
    };
    let async_token = if async_mode {
        quote! { async }
    } else {
        quote! {}
    };

    let implementation =
        generate_validator_batch_finalization_logic(&format_ident!("build"), &field_builder_chain, async_mode);
    let report_implementation = generate_validator_batch_finalization_logic(
        &format_ident!("build_report"),
        &field_builder_chain,
        async_mode,
    );
    let per_item_implementation = generate_validator_batch_per_item_finalization_logic(
        &format_ident!("build"),
        &per_item_builder_chain,
        async_mode,
    );
    let per_item_report_implementation = generate_validator_batch_per_item_finalization_logic(
        &format_ident!("build_report"),
        &per_item_builder_chain,
        async_mode,
    );
    let slot_state_idents = (0..field_names.len())
        .map(|idx| format_ident!("__STATUM_SLOT_{}_SET", idx))
        .collect::<Vec<_>>();
    let builder_defaults =
        builder_generics(&extra_machine_generics, false, &slot_state_idents, true);
    let builder_impl_generics_decl =
        builder_generics(&extra_machine_generics, false, &slot_state_idents, false);
    let (builder_impl_generics, builder_ty_generics, builder_where_clause) =
        builder_impl_generics_decl.split_for_impl();
    let initial_builder_slots = slot_state_idents
        .iter()
        .map(|_| quote! { false })
        .collect::<Vec<_>>();
    let initial_builder_ty_generics =
        generic_argument_tokens(extra_machine_generics.params.iter(), None, &initial_builder_slots);
    let complete_builder_slots = slot_state_idents
        .iter()
        .map(|_| quote! { true })
        .collect::<Vec<_>>();
    let complete_builder_ty_generics =
        generic_argument_tokens(extra_machine_generics.params.iter(), None, &complete_builder_slots);
    let complete_builder_impl_generics_decl =
        builder_generics(&extra_machine_generics, false, &[], false);
    let (complete_builder_impl_generics, _, complete_builder_where_clause) =
        complete_builder_impl_generics_decl.split_for_impl();
    let shared_builder_where_clause = validator_support_merged_where_clause_tokens(
        complete_builder_where_clause,
        field_types
            .iter()
            .map(|field_type| quote! { #field_type: core::clone::Clone })
            .collect(),
    );
    let by_builder_decl_generics =
        validator_support_prefixed_generics_declaration_tokens("F", &extra_machine_generics);
    let by_builder_ty_generics = validator_support_prefixed_generics_argument_tokens(
        quote! { F },
        extra_machine_generics.params.iter(),
    );
    let by_builder_where_clause = validator_support_merged_where_clause_tokens(
        extra_machine_generics.where_clause.as_ref(),
        vec![quote! { F: Fn(&#persisted_ty) -> #fields_ty }],
    );
    let by_builder_marker_field = if extra_machine_generics.params.is_empty() {
        quote! {}
    } else {
        let marker_ty = validator_support_generic_usage_marker_tokens(&extra_machine_generics);
        quote! {
            __statum_marker: core::marker::PhantomData<fn() -> #marker_ty>,
        }
    };
    let by_builder_marker_init = if extra_machine_generics.params.is_empty() {
        quote! {}
    } else {
        quote! {
            __statum_marker: core::marker::PhantomData,
        }
    };
    let field_storage = field_names.iter().zip(field_types.iter()).map(|(field_name, field_type)| {
        quote! { #field_name: core::option::Option<#field_type> }
    });
    let builder_init = field_names.iter().map(|field_name| {
        quote! { #field_name: core::option::Option::None }
    });
    let field_bindings = field_names
        .iter()
        .map(|field_name| {
            let message = format!("statum internal error: `{field_name}` was not set before build");
            quote! {
                let #field_name = self.#field_name.expect(#message);
            }
        })
        .collect::<Vec<_>>();
    let setters = field_names
        .iter()
        .zip(field_types.iter())
        .enumerate()
        .map(|(slot_idx, (field_name, field_type))| {
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
            let target_generics =
                generic_argument_tokens(extra_machine_generics.params.iter(), None, &generics);
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
                        __statum_items: self.__statum_items,
                        #(#assignments),*
                    }
                }
            }
        });

    quote! {
        impl #into_machines_impl_generics #machine_module_ident::IntoMachinesExt #into_machines_trait_args for T
        #into_machines_where_clause
        {
            type Builder = #builder_ident #initial_builder_ty_generics;
            type BuilderWithFields<F> = #by_builder_ident #by_builder_ty_generics;

            fn into_machines(self) -> Self::Builder {
                #builder_ident {
                    __statum_items: self.into(),
                    #(#builder_init),*
                }
            }

            fn into_machines_by<F>(self, fields: F) -> Self::BuilderWithFields<F>
            where
                F: Fn(&#persisted_ty) -> #fields_ty,
            {
                #by_builder_ident {
                    __statum_items: self.into(),
                    __statum_fields_fn: fields,
                    #by_builder_marker_init
                }
            }
        }

        #[doc(hidden)]
        #[allow(dead_code, private_bounds, private_interfaces)]
        #machine_vis struct #builder_ident #builder_defaults {
            __statum_items: Vec<#persisted_ty>,
            #(#field_storage),*
        }

        #[allow(dead_code, private_bounds, private_interfaces)]
        impl #builder_impl_generics #builder_ident #builder_ty_generics #builder_where_clause {
            #(#setters)*
        }

        #[allow(dead_code, private_bounds, private_interfaces)]
        impl #complete_builder_impl_generics #builder_ident #complete_builder_ty_generics #shared_builder_where_clause {
            #[inline(always)]
            #machine_vis #async_token fn build(self) -> Vec<core::result::Result<#machine_state_ty, statum::Error>> {
                let __statum_items = self.__statum_items;
                #(#field_bindings)*
                #implementation
            }

            #[inline(always)]
            #machine_vis #async_token fn build_reports(self) -> Vec<statum::RebuildReport<#machine_state_ty>> {
                let __statum_items = self.__statum_items;
                #(#field_bindings)*
                #report_implementation
            }
        }

        #[doc(hidden)]
        #[allow(dead_code, private_bounds, private_interfaces)]
        #machine_vis struct #by_builder_ident #by_builder_decl_generics {
            __statum_items: Vec<#persisted_ty>,
            __statum_fields_fn: F,
            #by_builder_marker_field
        }

        #[allow(dead_code, private_bounds, private_interfaces)]
        impl #by_builder_decl_generics #by_builder_ident #by_builder_ty_generics
        #by_builder_where_clause
        {
            #[inline(always)]
            #machine_vis #async_token fn build(self) -> Vec<core::result::Result<#machine_state_ty, statum::Error>> {
                self.__private_finalize()#await_token
            }

            #[inline(always)]
            #machine_vis #async_token fn build_reports(self) -> Vec<statum::RebuildReport<#machine_state_ty>> {
                self.__private_finalize_reports()#await_token
            }

            #async_token fn __private_finalize(self) -> Vec<core::result::Result<#machine_state_ty, statum::Error>> {
                #per_item_implementation
            }

            #async_token fn __private_finalize_reports(self) -> Vec<statum::RebuildReport<#machine_state_ty>> {
                #per_item_report_implementation
            }
        }
    }
}

fn generate_validator_batch_finalization_logic(
    builder_method: &Ident,
    field_builder_chain: &TokenStream,
    async_mode: bool,
) -> TokenStream {
    if async_mode {
        quote! {
            statum::__private::futures::future::join_all(
                __statum_items.iter().map(|__statum_item| {
                    __statum_item.into_machine()
                        #field_builder_chain
                        .#builder_method()
                })
            ).await
        }
    } else {
        quote! {
            __statum_items
                .into_iter()
                .map(|__statum_item| {
                    __statum_item.into_machine()
                        #field_builder_chain
                        .#builder_method()
                })
                .collect()
        }
    }
}

fn generate_validator_batch_per_item_finalization_logic(
    builder_method: &Ident,
    field_builder_chain: &TokenStream,
    async_mode: bool,
) -> TokenStream {
    if async_mode {
        quote! {
            let __statum_field_fn = &self.__statum_fields_fn;
            statum::__private::futures::future::join_all(
                self.__statum_items.iter().map(|__statum_item| {
                    let __statum_fields = __statum_field_fn(__statum_item);
                    __statum_item.into_machine()
                        #field_builder_chain
                        .#builder_method()
                })
            ).await
        }
    } else {
        quote! {
            let __statum_field_fn = self.__statum_fields_fn;
            self.__statum_items
                .into_iter()
                .map(|__statum_item| {
                    let __statum_fields = __statum_field_fn(&__statum_item);
                    __statum_item.into_machine()
                        #field_builder_chain
                        .#builder_method()
                })
                .collect()
        }
    }
}

fn validator_support_generic_usage_marker_tokens(generics: &Generics) -> TokenStream {
    let args = generics
        .params
        .iter()
        .map(validator_support_generic_marker_type_token)
        .collect::<Vec<_>>();
    match args.as_slice() {
        [] => quote! { () },
        [arg] => quote! { #arg },
        _ => quote! { (#(#args),*) },
    }
}

fn validator_support_prefixed_generics_declaration_tokens(
    prefix_ident: &str,
    generics: &Generics,
) -> TokenStream {
    let prefix_ident = format_ident!("{prefix_ident}");
    if generics.params.is_empty() {
        quote! { <#prefix_ident> }
    } else {
        let params = generics.params.iter();
        quote! { <#prefix_ident, #(#params),*> }
    }
}

fn validator_support_prefixed_generics_argument_tokens<'a>(
    prefix: TokenStream,
    params: impl Iterator<Item = &'a GenericParam>,
) -> TokenStream {
    let args = params
        .map(validator_support_generic_argument_token)
        .collect::<Vec<_>>();
    if args.is_empty() {
        quote! { <#prefix> }
    } else {
        quote! { <#prefix, #(#args),*> }
    }
}

fn validator_support_merged_where_clause_tokens(
    where_clause: Option<&syn::WhereClause>,
    extra_predicates: Vec<TokenStream>,
) -> TokenStream {
    let predicates = where_clause
        .iter()
        .flat_map(|clause| clause.predicates.iter())
        .map(|predicate| quote! { #predicate })
        .chain(extra_predicates)
        .collect::<Vec<_>>();

    if predicates.is_empty() {
        quote! {}
    } else {
        quote! { where #(#predicates),* }
    }
}

fn validator_support_generic_argument_token(param: &GenericParam) -> TokenStream {
    match param {
        GenericParam::Lifetime(lifetime) => {
            let lifetime = &lifetime.lifetime;
            quote! { #lifetime }
        }
        GenericParam::Type(ty) => {
            let ident = &ty.ident;
            quote! { #ident }
        }
        GenericParam::Const(const_param) => {
            let ident = &const_param.ident;
            quote! { #ident }
        }
    }
}

fn validator_support_generic_marker_type_token(param: &GenericParam) -> TokenStream {
    match param {
        GenericParam::Lifetime(lifetime) => {
            let lifetime = &lifetime.lifetime;
            quote! { &#lifetime () }
        }
        GenericParam::Type(ty) => {
            let ident = &ty.ident;
            quote! { #ident }
        }
        GenericParam::Const(const_param) => {
            let ident = &const_param.ident;
            quote! { [(); #ident] }
        }
    }
}

fn with_appended_where_clause(
    where_clause: Option<&syn::WhereClause>,
    predicate: TokenStream,
) -> TokenStream {
    match where_clause {
        Some(where_clause) => quote! { #where_clause, #predicate },
        None => quote! { where #predicate },
    }
}

fn generate_machine_state_surface(
    machine_info: &MachineInfo,
    parsed_machine: &ParsedMachineInfo,
    machine_ident: &Ident,
) -> Result<TokenStream, TokenStream> {
    let extra_machine_generics = extra_generics(&parsed_machine.generics);
    let extra_ty_args = extra_type_arguments_tokens(&parsed_machine.generics);
    let extra_impl_generics = extra_machine_generics.clone();
    let (_, some_state_ty_generics, some_state_where_clause) = extra_impl_generics.split_for_impl();
    let fields_struct_fields = parsed_machine.fields.iter().map(|field| {
        let field_ident = &field.ident;
        let alias_ident = format_ident!(
            "{}",
            field_type_alias_name(&machine_info.name, &field.ident.to_string())
        );
        quote! {
            pub #field_ident: super::#alias_ident #extra_ty_args
        }
    });
    let vis = parsed_machine.vis.clone();
    let module_ident = machine_state_module_ident(machine_info);
    let introspection_surface = generate_machine_module_introspection(machine_info)?;
    let extra_params = extra_machine_generics.params.iter();
    let extra_where_clause = extra_machine_generics.where_clause.clone();
    let into_machines_trait = if extra_machine_generics.params.is_empty() {
        quote! {
            pub trait IntoMachinesExt<Item>: Sized {
                type Builder;
                type BuilderWithFields<F>;

                fn into_machines(self) -> Self::Builder;

                fn into_machines_by<F>(self, fields: F) -> Self::BuilderWithFields<F>
                where
                    F: Fn(&Item) -> Fields;
            }
        }
    } else {
        quote! {
            pub trait IntoMachinesExt<Item, #(#extra_params),*>: Sized
            #extra_where_clause
            {
                type Builder;
                type BuilderWithFields<F>;

                fn into_machines(self) -> Self::Builder;

                fn into_machines_by<F>(self, fields: F) -> Self::BuilderWithFields<F>
                where
                    F: Fn(&Item) -> Fields #extra_ty_args;
            }
        }
    };
    let state_machine_ty = machine_type_with_state(
        quote! { super::#machine_ident },
        &parsed_machine.generics,
        quote! { super::$variant },
    );

    Ok(quote! {
        #[allow(dead_code)]
        #vis mod #module_ident {
            #[allow(unused_imports)]
            use super::*;

            pub struct Fields #extra_machine_generics {
                #(#fields_struct_fields),*
            }

            #[allow(clippy::enum_variant_names)]
            pub enum SomeState #extra_machine_generics {
                $( $variant(#state_machine_ty) ),*
            }

            pub type State #extra_machine_generics = SomeState #extra_ty_args;

            #into_machines_trait

            impl #extra_impl_generics SomeState #some_state_ty_generics #some_state_where_clause {
                $(
                    pub fn $is_fn(&self) -> bool {
                        matches!(self, Self::$variant(_))
                    }
                )*
            }

            #introspection_surface
        }
    })
}

fn generate_machine_module_introspection(machine_info: &MachineInfo) -> Result<TokenStream, TokenStream> {
    let presentation_types = resolve_presentation_types(machine_info)?;
    let linked_state_entries = linked_state_entries(machine_info)?;
    let static_machine_link_entries = static_machine_link_entries(machine_info)?;
    let state_presentation_entry_macro_ident =
        machine_state_presentation_entry_macro_ident(machine_info);
    let transition_slice_ident = transition_slice_ident(
        &machine_info.name,
        machine_info.file_path.as_deref(),
        machine_info.line_number,
    );
    let transition_presentation_slice_ident = transition_presentation_slice_ident(
        &machine_info.name,
        machine_info.file_path.as_deref(),
        machine_info.line_number,
    );
    let linked_transition_slice_ident = linked_transition_slice_ident(
        &machine_info.name,
        machine_info.file_path.as_deref(),
        machine_info.line_number,
    );
    let module_path = LitStr::new(machine_info.module_path.as_ref(), Span::call_site());
    let rust_type_path = LitStr::new(
        &format!("{}::{}", machine_info.module_path, machine_info.name),
        Span::call_site(),
    );
    let machine_presentation = machine_presentation_tokens(
        machine_info.presentation.as_ref(),
        &machine_info.name,
        presentation_types.machine.as_ref(),
    )?;
    let machine_presentation_label =
        optional_lit_str_tokens(machine_info.presentation.as_ref().and_then(|value| value.label.as_deref()));
    let machine_presentation_description = optional_lit_str_tokens(
        machine_info
            .presentation
            .as_ref()
            .and_then(|value| value.description.as_deref()),
    );
    let machine_meta_ty = presentation_type_tokens(presentation_types.machine.as_ref());
    let state_meta_ty = presentation_type_tokens(presentation_types.state.as_ref());
    let transition_meta_ty = presentation_type_tokens(presentation_types.transition.as_ref());

    Ok(quote! {
        #[allow(clippy::enum_variant_names)]
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
        pub enum StateId {
            $( $variant ),*
        }

        #[derive(Clone, Copy)]
        pub struct TransitionId(&'static statum::__private::TransitionToken);

        impl TransitionId {
            #[doc(hidden)]
            pub const fn from_token(token: &'static statum::__private::TransitionToken) -> Self {
                Self(token)
            }
        }

        impl core::fmt::Debug for TransitionId {
            fn fmt(
                &self,
                formatter: &mut core::fmt::Formatter<'_>,
            ) -> core::result::Result<(), core::fmt::Error> {
                formatter.write_str("TransitionId(..)")
            }
        }

        impl core::cmp::PartialEq for TransitionId {
            fn eq(&self, other: &Self) -> bool {
                core::ptr::eq(self.0, other.0)
            }
        }

        impl core::cmp::Eq for TransitionId {}

        impl core::hash::Hash for TransitionId {
            fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
                let ptr = core::ptr::from_ref(self.0) as usize;
                <usize as core::hash::Hash>::hash(&ptr, state);
            }
        }

        static __STATUM_STATES: &[statum::StateDescriptor<StateId>] = &[
            $(
                statum::StateDescriptor {
                    id: StateId::$variant,
                    rust_name: $rust_name,
                    has_data: $has_data,
                },
            )*
        ];

        static __STATUM_STATE_PRESENTATIONS:
            &[statum::__private::StatePresentation<StateId, #state_meta_ty>] =
            #state_presentation_entry_macro_ident! {
                $(
                    {
                        variant = $variant,
                        has_presentation = $has_presentation,
                        has_metadata = $has_metadata,
                        label = $label,
                        description = $description,
                        metadata = $metadata
                    }
                )*
            };

        static __STATUM_LINKED_STATES: &[statum::__private::LinkedStateDescriptor] = &[
            #(#linked_state_entries),*
        ];

        static __STATUM_STATIC_MACHINE_LINKS: &[statum::__private::StaticMachineLinkDescriptor] = &[
            #(#static_machine_link_entries),*
        ];

        #[doc(hidden)]
        #[statum::__private::linkme::distributed_slice]
        #[linkme(crate = statum::__private::linkme)]
        pub static #transition_slice_ident: [statum::TransitionDescriptor<StateId, TransitionId>];

        #[doc(hidden)]
        #[statum::__private::linkme::distributed_slice]
        #[linkme(crate = statum::__private::linkme)]
        pub static #transition_presentation_slice_ident:
            [statum::__private::TransitionPresentation<TransitionId, #transition_meta_ty>];

        #[doc(hidden)]
        #[statum::__private::linkme::distributed_slice]
        #[linkme(crate = statum::__private::linkme)]
        pub static #linked_transition_slice_ident: [statum::__private::LinkedTransitionDescriptor];

        #[doc(hidden)]
        #[allow(unused_imports)]
        pub use self::#transition_slice_ident as __STATUM_TRANSITIONS;

        #[doc(hidden)]
        #[allow(unused_imports)]
        pub use self::#transition_presentation_slice_ident as __STATUM_TRANSITION_PRESENTATIONS;

        #[doc(hidden)]
        #[allow(unused_imports)]
        pub use self::#linked_transition_slice_ident as __STATUM_LINKED_TRANSITIONS;

        #[doc(hidden)]
        pub type __StatumTransitionPresentationMetadata = #transition_meta_ty;

        fn __statum_transitions() -> &'static [statum::TransitionDescriptor<StateId, TransitionId>] {
            &#transition_slice_ident
        }

        pub static GRAPH: statum::MachineGraph<StateId, TransitionId> = statum::MachineGraph {
            machine: statum::MachineDescriptor {
                module_path: #module_path,
                rust_type_path: #rust_type_path,
            },
            states: __STATUM_STATES,
            transitions: statum::TransitionInventory::new(__statum_transitions),
        };

        fn __statum_transition_presentations(
        ) -> &'static [statum::__private::TransitionPresentation<TransitionId, #transition_meta_ty>] {
            &#transition_presentation_slice_ident
        }

        fn __statum_linked_transitions() -> &'static [statum::__private::LinkedTransitionDescriptor] {
            &#linked_transition_slice_ident
        }

        pub static PRESENTATION: statum::__private::MachinePresentation<
            StateId,
            TransitionId,
            #machine_meta_ty,
            #state_meta_ty,
            #transition_meta_ty,
        > = statum::__private::MachinePresentation {
            machine: #machine_presentation,
            states: __STATUM_STATE_PRESENTATIONS,
            transitions: statum::__private::TransitionPresentationInventory::new(
                __statum_transition_presentations,
            ),
        };

        #[doc(hidden)]
        #[statum::__private::linkme::distributed_slice(statum::__private::__STATUM_LINKED_MACHINES)]
        #[linkme(crate = statum::__private::linkme)]
        static __STATUM_LINKED_MACHINE: statum::__private::LinkedMachineGraph =
            statum::__private::LinkedMachineGraph {
                machine: statum::MachineDescriptor {
                    module_path: #module_path,
                    rust_type_path: #rust_type_path,
                },
                label: #machine_presentation_label,
                description: #machine_presentation_description,
                states: __STATUM_LINKED_STATES,
                transitions: statum::__private::LinkedTransitionInventory::new(__statum_linked_transitions),
                static_links: __STATUM_STATIC_MACHINE_LINKS,
            };
    })
}

fn linked_state_entries(machine_info: &MachineInfo) -> Result<Vec<TokenStream>, TokenStream> {
    let state_enum = machine_info.get_matching_state_enum()?;
    state_enum
        .variants
        .iter()
        .map(|variant| {
            let rust_name = LitStr::new(&variant.name, Span::call_site());
            let label = optional_lit_str_tokens(
                variant
                    .presentation
                    .as_ref()
                    .and_then(|value| value.label.as_deref()),
            );
            let description = optional_lit_str_tokens(
                variant
                    .presentation
                    .as_ref()
                    .and_then(|value| value.description.as_deref()),
            );
            let has_data = !matches!(variant.shape, crate::state::VariantShape::Unit);

            Ok(quote! {
                statum::__private::LinkedStateDescriptor {
                    rust_name: #rust_name,
                    label: #label,
                    description: #description,
                    has_data: #has_data,
                }
            })
        })
        .collect()
}

fn static_machine_link_entries(machine_info: &MachineInfo) -> Result<Vec<TokenStream>, TokenStream> {
    let state_enum = machine_info.get_matching_state_enum()?;
    let mut entries = Vec::new();

    for variant in &state_enum.variants {
        let from_state = LitStr::new(&variant.name, Span::call_site());
        match variant.parse_shape()? {
            crate::state::ParsedVariantShape::Unit => {}
            crate::state::ParsedVariantShape::Tuple { data_type } => {
                if let Some(candidate) = machine_link_candidate(data_type.as_ref()) {
                    let machine_path = candidate.machine_path.iter().map(|segment| {
                        let segment = LitStr::new(segment, Span::call_site());
                        quote! { #segment }
                    });
                    let to_state = LitStr::new(&candidate.state_name, Span::call_site());
                    entries.push(quote! {
                        statum::__private::StaticMachineLinkDescriptor {
                            from_state: #from_state,
                            field_name: None,
                            to_machine_path: &[#(#machine_path),*],
                            to_state: #to_state,
                        }
                    });
                }
            }
            crate::state::ParsedVariantShape::Named { fields, .. } => {
                for field in fields {
                    if let Some(candidate) = machine_link_candidate(&field.field_type) {
                        let field_name = LitStr::new(&field.ident.to_string(), Span::call_site());
                        let machine_path = candidate.machine_path.iter().map(|segment| {
                            let segment = LitStr::new(segment, Span::call_site());
                            quote! { #segment }
                        });
                        let to_state = LitStr::new(&candidate.state_name, Span::call_site());
                        entries.push(quote! {
                            statum::__private::StaticMachineLinkDescriptor {
                                from_state: #from_state,
                                field_name: Some(#field_name),
                                to_machine_path: &[#(#machine_path),*],
                                to_state: #to_state,
                            }
                        });
                    }
                }
            }
        }
    }

    Ok(entries)
}

struct MachineLinkCandidate {
    machine_path: Vec<String>,
    state_name: String,
}

fn machine_link_candidate(ty: &Type) -> Option<MachineLinkCandidate> {
    let Type::Path(TypePath { qself: None, path }) = ty else {
        return None;
    };
    let segment = path.segments.last()?;
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    let target_state = arguments.args.iter().find_map(|argument| match argument {
        GenericArgument::Type(ty) => machine_link_state_name(ty),
        _ => None,
    })?;
    let machine_path = normalized_machine_path(path);
    if machine_path.is_empty() {
        return None;
    }

    Some(MachineLinkCandidate {
        machine_path,
        state_name: target_state,
    })
}

fn machine_link_state_name(ty: &Type) -> Option<String> {
    let Type::Path(TypePath { qself: None, path }) = ty else {
        return None;
    };
    let segment = path.segments.last()?;
    matches!(segment.arguments, PathArguments::None).then(|| segment.ident.to_string())
}

fn normalized_machine_path(path: &syn::Path) -> Vec<String> {
    path.segments
        .iter()
        .skip_while(|segment| {
            matches!(
                segment.ident.to_string().as_str(),
                "crate" | "self" | "super"
            )
        })
        .map(|segment| segment.ident.to_string())
        .collect()
}

fn generate_state_presentation_entry_macro(
    machine_info: &MachineInfo,
    macro_ident: &Ident,
) -> Result<TokenStream, TokenStream> {
    let presentation_types = resolve_presentation_types(machine_info)?;
    Ok(state_presentation_entry_macro_tokens(
        macro_ident,
        presentation_types.state.as_ref(),
    ))
}

fn state_presentation_entry_macro_tokens(
    macro_ident: &Ident,
    state_meta_ty: Option<&syn::Type>,
) -> TokenStream {
    let state_type_hint = LitStr::new(presentation_type_hint("state"), Span::call_site());

    match state_meta_ty {
        Some(_) => quote! {
            macro_rules! #macro_ident {
                (@collect [$($out:tt)*]) => {
                    &[ $($out)* ]
                };
                (
                    @collect [$($out:tt)*]
                    {
                        variant = $variant:ident,
                        has_presentation = false,
                        has_metadata = false,
                        label = $label:expr,
                        description = $description:expr,
                        metadata = $metadata:tt
                    }
                    $($rest:tt)*
                ) => {
                    #macro_ident!(@collect [$($out)*] $($rest)*)
                };
                (
                    @collect [$($out:tt)*]
                    {
                        variant = $variant:ident,
                        has_presentation = true,
                        has_metadata = true,
                        label = $label:expr,
                        description = $description:expr,
                        metadata = ($metadata:expr)
                    }
                    $($rest:tt)*
                ) => {
                    #macro_ident!(@collect [
                        $($out)*
                        statum::__private::StatePresentation {
                            id: StateId::$variant,
                            label: $label,
                            description: $description,
                            metadata: $metadata,
                        },
                    ] $($rest)*)
                };
                (
                    @collect [$($out:tt)*]
                    {
                        variant = $variant:ident,
                        has_presentation = true,
                        has_metadata = false,
                        label = $label:expr,
                        description = $description:expr,
                        metadata = $metadata:tt
                    }
                    $($rest:tt)*
                ) => {
                    {
                        compile_error!(concat!(
                            "Error: `",
                            stringify!($variant),
                            "` uses `#[present(...)]`, and its machine declared `#[presentation_types(state = ...)]`.\nFix: add `metadata = ...` to that `#[present(...)]` attribute so the generated typed presentation surface has a value for every annotated state."
                        ));
                        &[]
                    }
                };
                ($($variants:tt)*) => {
                    #macro_ident!(@collect [] $($variants)*)
                };
            }
        },
        None => quote! {
            macro_rules! #macro_ident {
                (@collect [$($out:tt)*]) => {
                    &[ $($out)* ]
                };
                (
                    @collect [$($out:tt)*]
                    {
                        variant = $variant:ident,
                        has_presentation = false,
                        has_metadata = false,
                        label = $label:expr,
                        description = $description:expr,
                        metadata = $metadata:tt
                    }
                    $($rest:tt)*
                ) => {
                    #macro_ident!(@collect [$($out)*] $($rest)*)
                };
                (
                    @collect [$($out:tt)*]
                    {
                        variant = $variant:ident,
                        has_presentation = true,
                        has_metadata = false,
                        label = $label:expr,
                        description = $description:expr,
                        metadata = $metadata:tt
                    }
                    $($rest:tt)*
                ) => {
                    #macro_ident!(@collect [
                        $($out)*
                        statum::__private::StatePresentation {
                            id: StateId::$variant,
                            label: $label,
                            description: $description,
                            metadata: (),
                        },
                    ] $($rest)*)
                };
                (
                    @collect [$($out:tt)*]
                    {
                        variant = $variant:ident,
                        has_presentation = true,
                        has_metadata = true,
                        label = $label:expr,
                        description = $description:expr,
                        metadata = $metadata:tt
                    }
                    $($rest:tt)*
                ) => {
                    {
                        compile_error!(concat!(
                            "Error: `",
                            stringify!($variant),
                            "` uses `#[present(metadata = ...)]`, but no `#[presentation_types(state = ...)]` was declared on its machine.\nFix: add `#[presentation_types(state = ",
                            #state_type_hint,
                            ")]` to the `#[machine]` struct or remove the metadata expression."
                        ));
                        &[]
                    }
                };
                ($($variants:tt)*) => {
                    #macro_ident!(@collect [] $($variants)*)
                };
            }
        },
    }
}

fn generate_machine_introspection_impls(
    machine_info: &MachineInfo,
    parsed_machine: &ParsedMachineInfo,
    machine_ident: &Ident,
    state_generic_ident: &Ident,
) -> TokenStream {
    let module_ident = machine_state_module_ident(machine_info);
    let impl_generics = machine_impl_generics_tokens(parsed_machine, state_generic_ident);
    let extra_where_clause = extra_generics(&parsed_machine.generics).where_clause.clone();
    let self_ty = machine_type_with_state(
        quote! { #machine_ident },
        &parsed_machine.generics,
        quote! { #state_generic_ident },
    );
    let extra_machine_generics = extra_generics(&parsed_machine.generics);
    let extra_impl_generics = if extra_machine_generics.params.is_empty() {
        quote! {}
    } else {
        let extra_params = extra_machine_generics.params.iter();
        quote! { <#(#extra_params),*> }
    };
    let variant_machine_ty = machine_type_with_state(
        quote! { #machine_ident },
        &parsed_machine.generics,
        quote! { $variant },
    );

    quote! {
        impl #impl_generics statum::MachineIntrospection for #self_ty #extra_where_clause {
            type StateId = #module_ident::StateId;
            type TransitionId = #module_ident::TransitionId;

            const GRAPH: &'static statum::MachineGraph<Self::StateId, Self::TransitionId> =
                &#module_ident::GRAPH;
        }

        $(
            impl #extra_impl_generics statum::MachineStateIdentity
                for #variant_machine_ty #extra_where_clause
            {
                const STATE_ID: Self::StateId = #module_ident::StateId::$variant;
            }
        )*
    }
}

fn generate_field_type_aliases(machine_info: &MachineInfo, item: &ItemStruct) -> TokenStream {
    let alias_vis = &item.vis;
    let extra_generics = extra_generics(&item.generics);
    let helper_trait_ident = format_ident!(
        "__statum_{}_field_type_resolve",
        to_snake_case(&machine_info.name)
    );
    let helper_struct_ident = format_ident!(
        "__statum_{}_field_type_identity",
        to_snake_case(&machine_info.name)
    );
    let helper_tokens = if extra_generics.params.is_empty() {
        quote! {}
    } else {
        quote! {
            #[doc(hidden)]
            trait #helper_trait_ident {
                type Type;
            }

            #[doc(hidden)]
            struct #helper_struct_ident<__StatumUsed, __StatumFieldTy>(
                core::marker::PhantomData<fn() -> (__StatumUsed, __StatumFieldTy)>,
            );

            impl<__StatumUsed, __StatumFieldTy> #helper_trait_ident
                for #helper_struct_ident<__StatumUsed, __StatumFieldTy>
            {
                type Type = __StatumFieldTy;
            }
        }
    };
    let aliases = item.fields.iter().filter_map(|field| {
        let field_ident = field.ident.as_ref()?;
        let alias_ident =
            format_ident!("{}", field_type_alias_name(&machine_info.name, &field_ident.to_string()));
        let field_ty = &field.ty;
        let alias_tokens = if extra_generics.params.is_empty() {
            quote! { #field_ty }
        } else {
            let generic_usage = generic_usage_marker_tokens(&extra_generics);
            quote! {
                <#helper_struct_ident<#generic_usage, #field_ty> as #helper_trait_ident>::Type
            }
        };
        Some(quote! {
            #[doc(hidden)]
            #[allow(non_camel_case_types)]
            #alias_vis type #alias_ident #extra_generics = #alias_tokens;
        })
    });

    quote! {
        #helper_tokens
        #(#aliases)*
    }
}

fn generic_usage_marker_tokens(generics: &Generics) -> TokenStream {
    let usages = generics
        .params
        .iter()
        .map(|param| match param {
            GenericParam::Lifetime(lifetime) => {
                let lifetime = &lifetime.lifetime;
                quote! { &#lifetime () }
            }
            GenericParam::Type(ty) => {
                let ident = &ty.ident;
                quote! { #ident }
            }
            GenericParam::Const(const_param) => {
                let ident = &const_param.ident;
                quote! { [(); #ident] }
            }
        })
        .collect::<Vec<_>>();

    if usages.len() == 1 {
        usages.into_iter().next().unwrap()
    } else {
        quote! { (#(#usages),*) }
    }
}

pub(crate) fn transition_support_module_ident(machine_info: &MachineInfo) -> Ident {
    format_ident!(
        "__statum_{}_transition",
        to_snake_case(&machine_info.name)
    )
}

fn machine_state_module_ident(machine_info: &MachineInfo) -> Ident {
    format_ident!("{}", to_snake_case(&machine_info.name))
}

fn machine_presentation_tokens(
    presentation: Option<&PresentationAttr>,
    machine_name: &str,
    metadata_ty: Option<&syn::Type>,
) -> Result<TokenStream, TokenStream> {
    let Some(presentation) = presentation else {
        return Ok(quote! { None });
    };
    let label = optional_lit_str_tokens(presentation.label.as_deref());
    let description = optional_lit_str_tokens(presentation.description.as_deref());
    let metadata = presentation_metadata_tokens(
        Some(presentation),
        "machine",
        machine_name,
        metadata_ty,
        true,
    )?;

    Ok(quote! {
        Some(statum::__private::MachinePresentationDescriptor {
            label: #label,
            description: #description,
            metadata: #metadata,
        })
    })
}

fn optional_lit_str_tokens(value: Option<&str>) -> TokenStream {
    match value {
        Some(value) => {
            let lit = LitStr::new(value, Span::call_site());
            quote! { Some(#lit) }
        }
        None => quote! { None },
    }
}

struct ResolvedPresentationTypes {
    machine: Option<syn::Type>,
    state: Option<syn::Type>,
    transition: Option<syn::Type>,
}

fn resolve_presentation_types(
    machine_info: &MachineInfo,
) -> Result<ResolvedPresentationTypes, TokenStream> {
    let machine = machine_info
        .presentation_types
        .as_ref()
        .map(PresentationTypesAttr::parse_machine_type)
        .transpose()
        .map_err(|err| err.to_compile_error())?
        .flatten();
    let state = machine_info
        .presentation_types
        .as_ref()
        .map(PresentationTypesAttr::parse_state_type)
        .transpose()
        .map_err(|err| err.to_compile_error())?
        .flatten();
    let transition = machine_info
        .presentation_types
        .as_ref()
        .map(PresentationTypesAttr::parse_transition_type)
        .transpose()
        .map_err(|err| err.to_compile_error())?
        .flatten();

    Ok(ResolvedPresentationTypes {
        machine,
        state,
        transition,
    })
}

fn presentation_type_tokens(metadata_ty: Option<&syn::Type>) -> TokenStream {
    match metadata_ty {
        Some(ty) => quote! { #ty },
        None => quote! { () },
    }
}

fn presentation_metadata_tokens(
    presentation: Option<&PresentationAttr>,
    category: &str,
    subject: &str,
    metadata_ty: Option<&syn::Type>,
    require_when_present: bool,
) -> Result<TokenStream, TokenStream> {
    let Some(presentation) = presentation else {
        return Ok(quote! { () });
    };

    match (presentation.metadata.as_deref(), metadata_ty) {
        (Some(metadata_expr), Some(_)) => {
            let metadata = syn::parse_str::<syn::Expr>(metadata_expr)
                .map_err(|err| err.to_compile_error())?;
            Ok(quote! { #metadata })
        }
        (Some(_), None) => Err(
            syn::Error::new(
                Span::call_site(),
                format!(
                    "Error: `{subject}` uses `#[present(metadata = ...)]`, but no `#[presentation_types({category} = ...)]` was declared on its machine.\nFix: add `#[presentation_types({category} = {})]` to the `#[machine]` struct or remove the metadata expression.",
                    presentation_type_hint(category),
                ),
            )
            .to_compile_error(),
        ),
        (None, Some(_)) if require_when_present => Err(
            syn::Error::new(
                Span::call_site(),
                format!(
                    "Error: `{subject}` uses `#[present(...)]`, and its machine declared `#[presentation_types({category} = ...)]`.\nFix: add `metadata = ...` to that `#[present(...)]` attribute so the generated typed presentation surface has a value for every annotated {category}."
                ),
            )
            .to_compile_error(),
        ),
        _ => Ok(quote! { () }),
    }
}

fn presentation_type_hint(category: &str) -> &'static str {
    match category {
        "machine" => "MachineMeta",
        "state" => "StateMeta",
        "transition" => "TransitionMeta",
        _ => "PresentationMeta",
    }
}
