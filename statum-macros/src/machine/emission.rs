use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{GenericParam, Generics, Ident, ItemStruct, LitStr, Visibility};

use crate::state::{ParsedEnumInfo, ParsedVariantInfo, ParsedVariantShape};
use crate::{EnumInfo, PresentationAttr, PresentationTypesAttr, to_snake_case};

use super::metadata::{ParsedMachineInfo, field_type_alias_name, is_rust_analyzer};
use super::{
    MachineInfo, builder_generics, extra_generics, extra_type_arguments_tokens,
    generic_argument_tokens, machine_type_with_state, transition_presentation_slice_ident,
    transition_slice_ident,
};

pub fn generate_machine_impls(machine_info: &MachineInfo, item: &ItemStruct) -> proc_macro2::TokenStream {
    let state_enum = match machine_info.get_matching_state_enum() {
        Ok(enum_info) => enum_info,
        Err(err) => return err,
    };
    let parsed_state = match state_enum.parse() {
        Ok(parsed) => parsed,
        Err(err) => return err,
    };
    let parsed_machine = match machine_info.parse() {
        Ok(parsed) => parsed,
        Err(err) => return err,
    };
    let machine_ident = format_ident!("{}", machine_info.name);
    let generics = match parse_generics(&parsed_machine, &state_enum) {
        Ok(generics) => generics,
        Err(err) => return err,
    };
    let state_generic_ident = match extract_state_generic_ident(&generics) {
        Ok(ident) => ident,
        Err(err) => return err,
    };
    let struct_def =
        match generate_struct_definition(
            &parsed_machine,
            &machine_ident,
            &generics,
            &state_generic_ident,
            &state_enum.get_trait_name(),
        )
        {
            Ok(def) => def,
            Err(err) => return err,
        };
    let builder_methods = machine_info.generate_builder_methods(&parsed_machine, &parsed_state);
    let transition_support = transition_support(machine_info, &parsed_machine, &state_enum);
    let field_type_aliases = generate_field_type_aliases(machine_info, item);
    let machine_state_surface = match generate_machine_state_surface(
        machine_info,
        &parsed_machine,
        &parsed_state,
        &machine_ident,
    ) {
        Ok(surface) => surface,
        Err(err) => return err,
    };
    let introspection_impls = generate_machine_introspection_impls(
        machine_info,
        &state_enum,
        &generics,
        &parsed_state,
        &machine_ident,
    );

    quote! {
        #transition_support
        #field_type_aliases
        #struct_def
        #builder_methods
        #machine_state_surface
        #introspection_impls
    }
}

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

fn parse_generics(
    parsed_machine: &ParsedMachineInfo,
    state_enum: &EnumInfo,
) -> Result<Generics, TokenStream> {
    let mut generics = parsed_machine.generics.clone();
    let has_extra_generics = generics.params.len() > 1;

    let Some(first_param) = generics.params.first_mut() else {
        return Err(
            syn::Error::new(
                Span::call_site(),
                "Machine struct must have a state generic as its first type parameter.",
            )
            .to_compile_error(),
        );
    };

    let GenericParam::Type(first_type) = first_param else {
        return Err(
            syn::Error::new(
                Span::call_site(),
                "Machine state generic must be a type parameter.",
            )
            .to_compile_error(),
        );
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
        first_type.bounds.push(syn::parse_quote!(#state_trait_ident));
    }

    if !has_extra_generics {
        let default_state_ident = format_ident!("Uninitialized{}", state_enum.name);
        first_type.default = Some(syn::parse_quote!(#default_state_ident));
        first_type.eq_token = Some(syn::Token![=](Span::call_site()));
    }

    Ok(generics)
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

fn transition_support(
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

    quote! {
        #[doc(hidden)]
        mod #support_module_ident {
            use super::*;

            #transition_to_trait
            #transition_with_trait
        }

        #[allow(unused_imports)]
        use #support_module_ident::{TransitionTo as _, TransitionWith as _};
    }
}

fn generate_machine_state_surface(
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
    let introspection_surface =
        generate_machine_module_introspection(machine_info, parsed_state)?;
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

fn generate_machine_module_introspection(
    machine_info: &MachineInfo,
    parsed_state: &ParsedEnumInfo,
) -> Result<TokenStream, TokenStream> {
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
        .filter_map(|variant| variant.presentation.as_ref().map(|presentation| (variant, presentation)))
        .map(|(variant, presentation)| -> Result<TokenStream, TokenStream> {
        let variant_ident = format_ident!("{}", variant.name);
        let label = optional_lit_str_tokens(presentation.label.as_deref());
        let description = optional_lit_str_tokens(presentation.description.as_deref());
        let metadata = presentation_metadata_tokens(
            Some(presentation),
            "state",
            &variant.name,
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
    })
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

fn generate_machine_introspection_impls(
    machine_info: &MachineInfo,
    _state_enum: &EnumInfo,
    generics: &Generics,
    parsed_state: &ParsedEnumInfo,
    machine_ident: &Ident,
) -> TokenStream {
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

fn generate_struct_definition(
    parsed_machine: &ParsedMachineInfo,
    machine_ident: &Ident,
    generics: &Generics,
    state_generic_ident: &Ident,
    state_trait_ident: &Ident,
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
    let next_machine_ty = machine_type_with_state(quote! { #machine_ident }, generics, quote! { N });

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
                Self: statum::CanTransitionMap<N, Output = #next_machine_ty>,
                F: FnOnce(<Self as statum::CanTransitionMap<N>>::CurrentData) ->
                    <N as statum::StateMarker>::Data,
            {
                <Self as statum::CanTransitionMap<N>>::transition_map(self, f)
            }
        }
    })
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
    let builder_impl_generics_decl =
        builder_generics(&extra_generics, false, &slot_state_idents, false);
    let (builder_impl_generics, builder_ty_generics, builder_where_clause) =
        builder_impl_generics_decl.split_for_impl();
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
            #builder_vis fn #setter_ident(self, value: #slot_type) -> #variant_builder_ident #target_generics {
                #variant_builder_ident {
                    #(#assignments),*
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

        impl #builder_impl_generics #variant_builder_ident #builder_ty_generics #builder_where_clause {
            #(#setters)*
        }

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
        ParsedVariantShape::Tuple { data_type } => Some(data_type.clone()),
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
