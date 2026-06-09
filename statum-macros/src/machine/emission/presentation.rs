use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Generics, Ident, LitStr};

use crate::state::{ParsedEnumInfo, ParsedVariantShape};
use crate::{EnumInfo, PresentationAttr, PresentationTypesAttr, to_snake_case};

use super::super::metadata::{ParsedMachineInfo, field_type_alias_name};
use super::super::{
    MachineInfo, extra_generics, extra_type_arguments_tokens, machine_type_with_state,
    transition_presentation_slice_ident, transition_slice_ident,
};

pub(super) fn generate_machine_state_surface(
    machine_info: &MachineInfo,
    parsed_machine: &ParsedMachineInfo,
    parsed_state: &ParsedEnumInfo,
    machine_ident: &Ident,
) -> Result<TokenStream, TokenStream> {
    let extra_generics = extra_generics(&parsed_machine.generics);
    let extra_ty_args = extra_type_arguments_tokens(&parsed_machine.generics);
    let extra_impl_generics = extra_generics.clone();
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
    let state_variants = parsed_state.variants.iter().map(|variant| {
        let variant_ident = format_ident!("{}", variant.name);
        let state_machine_ty = machine_type_with_state(
            quote! { super::#machine_ident },
            &parsed_machine.generics,
            quote! { super::#variant_ident },
        );
        quote! {
            #variant_ident(#state_machine_ty)
        }
    });

    let vis = parsed_machine.vis.clone();
    let is_methods = parsed_state.variants.iter().map(|variant| {
        let variant_ident = format_ident!("{}", variant.name);
        let fn_name = format_ident!("is_{}", to_snake_case(&variant.name));
        quote! {
            pub fn #fn_name(&self) -> bool {
                matches!(self, Self::#variant_ident(_))
            }
        }
    });
    let module_ident = machine_state_module_ident(machine_info);
    let introspection_surface = generate_machine_module_introspection(machine_info, parsed_state)?;
    let extra_params = extra_generics.params.iter();
    let extra_where_clause = extra_generics.where_clause.clone();
    let into_machines_trait = if extra_generics.params.is_empty() {
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

    Ok(quote! {
        #vis mod #module_ident {
            #[allow(unused_imports)]
            use super::*;

            pub struct Fields #extra_generics {
                #(#fields_struct_fields),*
            }

            pub enum SomeState #extra_generics {
                #(#state_variants),*
            }

            pub type State #extra_generics = SomeState #extra_ty_args;

            #into_machines_trait

            impl #extra_impl_generics SomeState #some_state_ty_generics #some_state_where_clause {
                #(#is_methods)*
            }

            #introspection_surface
        }
    })
}

fn machine_state_module_ident(machine_info: &MachineInfo) -> Ident {
    format_ident!("{}", to_snake_case(&machine_info.name))
}

fn generate_machine_module_introspection(
    machine_info: &MachineInfo,
    parsed_state: &ParsedEnumInfo,
) -> Result<TokenStream, TokenStream> {
    if !cfg!(feature = "introspection") {
        return Ok(quote! {});
    }

    let presentation_types = resolve_presentation_types(machine_info)?;
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
    let state_id_variants = parsed_state.variants.iter().map(|variant| {
        let variant_ident = format_ident!("{}", variant.name);
        quote! { #variant_ident }
    });
    let state_descriptors = parsed_state.variants.iter().map(|variant| {
        let variant_ident = format_ident!("{}", variant.name);
        let rust_name = LitStr::new(&variant.name, Span::call_site());
        let has_data = !matches!(variant.shape, ParsedVariantShape::Unit);
        quote! {
            statum::StateDescriptor {
                id: StateId::#variant_ident,
                rust_name: #rust_name,
                has_data: #has_data,
            }
        }
    });
    let module_path = LitStr::new(machine_info.module_path.as_ref(), Span::call_site());
    let rust_type_path = LitStr::new(
        &format!("{}::{}", machine_info.module_path, machine_info.name),
        Span::call_site(),
    );
    let state_count = parsed_state.variants.len();
    let state_presentations = parsed_state
        .variants
        .iter()
        .filter_map(|variant| {
            variant
                .presentation
                .as_ref()
                .map(|presentation| (variant, presentation))
        })
        .map(
            |(variant, presentation)| -> Result<TokenStream, TokenStream> {
                let variant_ident = format_ident!("{}", variant.name);
                let label = optional_lit_str_tokens(presentation.label.as_deref());
                let description = optional_lit_str_tokens(presentation.description.as_deref());
                let state_enum_name = machine_info
                    .state_generic_name
                    .as_deref()
                    .unwrap_or("<state>");
                let subject = format!("state `{state_enum_name}::{}`", variant.name);
                let metadata_display = presentation
                    .metadata
                    .as_ref()
                    .map(|metadata| format!("`#[present(metadata = {metadata})]`"));
                let metadata = presentation_metadata_tokens(
                    Some(presentation),
                    "state",
                    subject.as_str(),
                    metadata_display.as_deref(),
                    &machine_info.name,
                    presentation_types.state.as_ref(),
                    true,
                )?;

                Ok(quote! {
                    statum::__private::StatePresentation {
                        id: StateId::#variant_ident,
                        label: #label,
                        description: #description,
                        metadata: #metadata,
                    }
                })
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    let state_presentation_count = state_presentations.len();
    let machine_presentation = machine_presentation_tokens(
        machine_info.presentation.as_ref(),
        &machine_info.name,
        presentation_types.machine.as_ref(),
    )?;
    let machine_meta_ty = presentation_type_tokens(presentation_types.machine.as_ref());
    let state_meta_ty = presentation_type_tokens(presentation_types.state.as_ref());
    let transition_meta_ty = presentation_type_tokens(presentation_types.transition.as_ref());

    Ok(quote! {
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
        pub enum StateId {
            #(#state_id_variants),*
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

        static __STATUM_STATES: [statum::StateDescriptor<StateId>; #state_count] = [
            #(#state_descriptors),*
        ];

        static __STATUM_STATE_PRESENTATIONS:
            [statum::__private::StatePresentation<StateId, #state_meta_ty>; #state_presentation_count] = [
                #(#state_presentations),*
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

        fn __statum_transitions() -> &'static [statum::TransitionDescriptor<StateId, TransitionId>] {
            &#transition_slice_ident
        }

        pub static GRAPH: statum::MachineGraph<StateId, TransitionId> = statum::MachineGraph {
            machine: statum::MachineDescriptor {
                module_path: #module_path,
                rust_type_path: #rust_type_path,
            },
            states: &__STATUM_STATES,
            transitions: statum::TransitionInventory::new(__statum_transitions),
        };

        fn __statum_transition_presentations(
        ) -> &'static [statum::__private::TransitionPresentation<TransitionId, #transition_meta_ty>] {
            &#transition_presentation_slice_ident
        }

        pub static PRESENTATION: statum::__private::MachinePresentation<
            StateId,
            TransitionId,
            #machine_meta_ty,
            #state_meta_ty,
            #transition_meta_ty,
        > = statum::__private::MachinePresentation {
                machine: #machine_presentation,
                states: &__STATUM_STATE_PRESENTATIONS,
                transitions: statum::__private::TransitionPresentationInventory::new(
                    __statum_transition_presentations,
                ),
            };
    })
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
        format!("machine `{machine_name}`").as_str(),
        presentation
            .metadata
            .as_ref()
            .map(|metadata| format!("`#[present(metadata = {metadata})]`"))
            .as_deref(),
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
    metadata_display: Option<&str>,
    machine_name: &str,
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
                    "Error: {subject} uses `#[present(metadata = ...)]`, but machine `{machine_name}` did not declare `#[presentation_types({category} = ...)]`.\nFound: {}\nExpected: `#[presentation_types({category} = {})]` on machine `{machine_name}`\nFix: add `#[presentation_types({category} = {})]` to the `#[machine]` struct or remove the metadata expression.",
                    metadata_display.unwrap_or("`#[present(metadata = ...)]`"),
                    presentation_type_hint(category),
                    presentation_type_hint(category),
                ),
            )
            .to_compile_error(),
        ),
        (None, Some(_)) if require_when_present => Err(
            syn::Error::new(
                Span::call_site(),
                format!(
                    "Error: {subject} uses `#[present(...)]`, and machine `{machine_name}` declared `#[presentation_types({category} = ...)]`.\nFix: add `metadata = ...` to that `#[present(...)]` attribute so the generated typed presentation surface has a value for every annotated {category}."
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

pub(super) fn generate_machine_introspection_impls(
    machine_info: &MachineInfo,
    _state_enum: &EnumInfo,
    generics: &Generics,
    parsed_state: &ParsedEnumInfo,
    machine_ident: &Ident,
) -> TokenStream {
    if !cfg!(feature = "introspection") {
        return quote! {};
    }

    let module_ident = machine_state_module_ident(machine_info);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let extra_generics = extra_generics(generics);
    let (extra_impl_generics, _, extra_where_clause) = extra_generics.split_for_impl();
    let state_identity_impls = parsed_state.variants.iter().map(|variant| {
        let variant_ident = format_ident!("{}", variant.name);
        let machine_ty = machine_type_with_state(
            quote! { #machine_ident },
            generics,
            quote! { #variant_ident },
        );
        quote! {
            impl #extra_impl_generics statum::MachineStateIdentity for #machine_ty #extra_where_clause {
                const STATE_ID: Self::StateId = #module_ident::StateId::#variant_ident;
            }
        }
    });

    quote! {
        impl #impl_generics statum::MachineIntrospection for #machine_ident #ty_generics #where_clause {
            type StateId = #module_ident::StateId;
            type TransitionId = #module_ident::TransitionId;

            const GRAPH: &'static statum::MachineGraph<Self::StateId, Self::TransitionId> =
                &#module_ident::GRAPH;
        }

        #(#state_identity_impls)*
    }
}
