use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{GenericParam, Generics, Ident, ItemStruct};

use crate::{EnumInfo, to_snake_case};

use super::super::metadata::{ParsedMachineInfo, field_type_alias_name};
use super::super::{MachineInfo, extra_generics, machine_type_with_state};

pub(super) fn parse_generics(
    parsed_machine: &ParsedMachineInfo,
    state_enum: &EnumInfo,
) -> Result<Generics, TokenStream> {
    let mut generics = parsed_machine.generics.clone();
    let has_extra_generics = generics.params.len() > 1;

    let Some(first_param) = generics.params.first_mut() else {
        return Err(syn::Error::new(
            Span::call_site(),
            "Machine struct must have a state generic as its first type parameter.",
        )
        .to_compile_error());
    };

    let GenericParam::Type(first_type) = first_param else {
        return Err(syn::Error::new(
            Span::call_site(),
            "Machine state generic must be a type parameter.",
        )
        .to_compile_error());
    };

    let state_trait_ident = state_enum.get_trait_name();
    let has_state_trait_bound = first_type.bounds.iter().any(|bound| {
        matches!(
            bound,
            syn::TypeParamBound::Trait(trait_bound)
            if trait_bound.path.is_ident(&state_trait_ident)
        )
    });
    if !has_state_trait_bound {
        first_type
            .bounds
            .push(syn::parse_quote!(#state_trait_ident));
    }

    if !has_extra_generics {
        let default_state_ident = format_ident!("Uninitialized{}", state_enum.name);
        first_type.default = Some(syn::parse_quote!(#default_state_ident));
        first_type.eq_token = Some(syn::Token![=](Span::call_site()));
    }

    Ok(generics)
}

pub(super) fn extract_state_generic_ident(generics: &Generics) -> Result<Ident, TokenStream> {
    let Some(first_param) = generics.params.first() else {
        return Err(syn::Error::new(
            Span::call_site(),
            "Machine struct must have a state generic as its first type parameter.",
        )
        .to_compile_error());
    };

    if let GenericParam::Type(first_type) = first_param {
        return Ok(first_type.ident.clone());
    }

    Err(syn::Error::new(
        Span::call_site(),
        "Machine state generic must be a type parameter.",
    )
    .to_compile_error())
}

pub(super) fn transition_support(
    machine_info: &MachineInfo,
    parsed_machine: &ParsedMachineInfo,
    state_enum: &EnumInfo,
) -> TokenStream {
    let trait_name = state_enum.get_trait_name();
    let machine_ident = format_ident!("{}", machine_info.name);
    let support_module_ident = transition_support_module_ident(machine_info);
    let extra_generics = extra_generics(&parsed_machine.generics);
    let extra_params = extra_generics.params.iter().cloned().collect::<Vec<_>>();
    let extra_where_clause = extra_generics.where_clause.clone();
    let next_machine_ty = machine_type_with_state(
        quote! { #machine_ident },
        &parsed_machine.generics,
        quote! { N },
    );
    let next_state_machine_ty = machine_type_with_state(
        quote! { #machine_ident },
        &parsed_machine.generics,
        quote! { Self::NextState },
    );
    let transition_to_trait = if extra_generics.params.is_empty() {
        quote! {
            pub trait TransitionTo<N: #trait_name> {
                fn transition(self) -> #next_machine_ty;
            }
        }
    } else {
        quote! {
            pub trait TransitionTo<N: #trait_name, #(#extra_params),*>
            #extra_where_clause
            {
                fn transition(self) -> #next_machine_ty;
            }
        }
    };
    let transition_with_trait = if extra_generics.params.is_empty() {
        quote! {
            pub trait TransitionWith<T> {
                type NextState: #trait_name;

                fn transition_with(
                    self,
                    data: T,
                ) -> #next_state_machine_ty;
            }
        }
    } else {
        quote! {
            pub trait TransitionWith<T, #(#extra_params),*>
            #extra_where_clause
            {
                type NextState: #trait_name;

                fn transition_with(
                    self,
                    data: T,
                ) -> #next_state_machine_ty;
            }
        }
    };
    let declared_transition_map_edge_trait = if extra_generics.params.is_empty() {
        quote! {
            pub trait DeclaredTransitionMapEdge<N: #trait_name + statum::StateMarker> {
                type CurrentData;

                fn transition_map<F>(self, f: F) -> #next_machine_ty
                where
                    F: FnOnce(Self::CurrentData) -> <N as statum::StateMarker>::Data;
            }
        }
    } else {
        quote! {
            pub trait DeclaredTransitionMapEdge<N: #trait_name + statum::StateMarker, #(#extra_params),*>
            #extra_where_clause
            {
                type CurrentData;

                fn transition_map<F>(self, f: F) -> #next_machine_ty
                where
                    F: FnOnce(Self::CurrentData) -> <N as statum::StateMarker>::Data;
            }
        }
    };

    quote! {
        #[doc(hidden)]
        mod #support_module_ident {
            use super::*;

            #transition_to_trait
            #transition_with_trait
            #declared_transition_map_edge_trait
        }

        #[allow(unused_imports)]
        use #support_module_ident::{DeclaredTransitionMapEdge as _, TransitionTo as _, TransitionWith as _};
    }
}

pub(super) fn generate_field_type_aliases(
    machine_info: &MachineInfo,
    item: &ItemStruct,
) -> TokenStream {
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
        let alias_ident = format_ident!(
            "{}",
            field_type_alias_name(&machine_info.name, &field_ident.to_string())
        );
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
    format_ident!("__statum_{}_transition", to_snake_case(&machine_info.name))
}

pub(super) fn generate_struct_definition(
    parsed_machine: &ParsedMachineInfo,
    machine_ident: &Ident,
    generics: &Generics,
    state_generic_ident: &Ident,
    state_trait_ident: &Ident,
    support_module_ident: &Ident,
) -> Result<TokenStream, TokenStream> {
    let mut field_tokens = Vec::with_capacity(parsed_machine.fields.len());
    for field in &parsed_machine.fields {
        let field_ident = &field.ident;
        let field_vis = &field.vis;
        let field_ty = &field.field_type;
        field_tokens.push(quote! { #field_vis #field_ident: #field_ty });
    }

    let derives = if parsed_machine.derives.is_empty() {
        quote! {}
    } else {
        let derive_tokens = parsed_machine.derives.clone();
        quote! {
            #[derive(#(#derive_tokens),*)]
        }
    };

    let vis = parsed_machine.vis.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let next_machine_ty =
        machine_type_with_state(quote! { #machine_ident }, generics, quote! { N });
    let extra_generics = extra_generics(generics);
    let extra_ty_args = extra_generics
        .params
        .iter()
        .map(|param| match param {
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
        })
        .collect::<Vec<TokenStream>>();
    let transition_map_trait_generics = if extra_ty_args.is_empty() {
        quote! { <N> }
    } else {
        quote! { <N, #(#extra_ty_args),*> }
    };

    Ok(quote! {
        #derives
        #vis struct #machine_ident #generics {
            marker: core::marker::PhantomData<#state_generic_ident>,
            pub state_data: #state_generic_ident::Data,
            #( #field_tokens ),*
        }

        impl #impl_generics #machine_ident #ty_generics #where_clause {
            #vis fn transition_map<N, F>(self, f: F) -> #next_machine_ty
            where
                N: #state_trait_ident + statum::StateMarker,
                Self: #support_module_ident::DeclaredTransitionMapEdge #transition_map_trait_generics,
                F: FnOnce(<Self as #support_module_ident::DeclaredTransitionMapEdge #transition_map_trait_generics>::CurrentData) ->
                    <N as statum::StateMarker>::Data,
            {
                <Self as #support_module_ident::DeclaredTransitionMapEdge #transition_map_trait_generics>::transition_map(self, f)
            }
        }
    })
}
