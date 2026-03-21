use macro_registry::analysis::{StructEntry, get_file_analysis};
use macro_registry::callsite::module_path_for_line;
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{Fields, Ident, Type, TypePath};

use crate::state::{ParsedEnumInfo, ParsedVariantInfo};

use super::MachineInfo;

pub(crate) fn generate_child_state_surface(
    machine_info: &MachineInfo,
    state_enum_name: &str,
    parsed_state: &ParsedEnumInfo,
    machine_ident: &Ident,
    state_trait_ident: &Ident,
    machine_field_names: &[Ident],
) -> Result<Option<TokenStream>, TokenStream> {
    let child_support = parsed_state
        .variants
        .iter()
        .map(|variant| analyze_child_variant(machine_info, state_enum_name, variant))
        .collect::<Result<Vec<_>, _>>()?;
    let child_support = child_support.into_iter().flatten().collect::<Vec<_>>();

    if child_support.is_empty() {
        return Ok(None);
    }

    let context_structs = child_support.iter().filter_map(ChildSupport::context_struct_tokens);
    let child_impls = child_support
        .iter()
        .map(|support| support.impl_tokens(machine_ident, state_trait_ident, machine_field_names));

    Ok(Some(quote! {
        pub trait ChildExt: Sized {
            type ChildMachine;
            type ChildContext;
            type ParentData;

            fn child(&self) -> &Self::ChildMachine;

            fn map_child<F>(self, f: F) -> Self
            where
                F: FnOnce(Self::ChildMachine) -> Self::ChildMachine;

            fn transition_map_child<N, F>(self, f: F) -> super::#machine_ident<N>
            where
                N: super::#state_trait_ident + statum::StateMarker,
                Self: statum::CanTransitionMap<
                    N,
                    CurrentData = Self::ParentData,
                    Output = super::#machine_ident<N>,
                >,
                F: FnOnce(Self::ChildMachine, Self::ChildContext) -> <N as statum::StateMarker>::Data;
        }

        #(#context_structs)*
        #(#child_impls)*
    }))
}

#[derive(Clone)]
struct ContextField {
    ident: Ident,
    ty: Type,
}

enum ChildSupport {
    Direct {
        variant_ident: Ident,
        child_ty: Type,
    },
    Wrapper {
        variant_ident: Ident,
        payload_ty: Type,
        child_ty: Type,
        child_field_ident: Ident,
        context_struct_ident: Ident,
        context_fields: Vec<ContextField>,
    },
}

impl ChildSupport {
    fn context_struct_tokens(&self) -> Option<TokenStream> {
        let Self::Wrapper {
            context_struct_ident,
            context_fields,
            ..
        } = self
        else {
            return None;
        };

        let field_tokens = context_fields.iter().map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            quote! {
                pub #ident: #ty
            }
        });

        Some(quote! {
            pub struct #context_struct_ident {
                #(#field_tokens),*
            }
        })
    }

    fn impl_tokens(
        &self,
        machine_ident: &Ident,
        state_trait_ident: &Ident,
        machine_field_names: &[Ident],
    ) -> TokenStream {
        match self {
            Self::Direct {
                variant_ident,
                child_ty,
            } => {
                quote! {
                    impl ChildExt for super::#machine_ident<super::#variant_ident> {
                        type ChildMachine = #child_ty;
                        type ChildContext = ();
                        type ParentData = #child_ty;

                        fn child(&self) -> &Self::ChildMachine {
                            &self.state_data
                        }

                        fn map_child<F>(self, f: F) -> Self
                        where
                            F: FnOnce(Self::ChildMachine) -> Self::ChildMachine,
                        {
                            let super::#machine_ident {
                                marker,
                                state_data,
                                #(#machine_field_names),*
                            } = self;

                            super::#machine_ident {
                                marker,
                                state_data: f(state_data),
                                #(#machine_field_names),*
                            }
                        }

                        fn transition_map_child<N, F>(self, f: F) -> super::#machine_ident<N>
                        where
                            N: super::#state_trait_ident + statum::StateMarker,
                            Self: statum::CanTransitionMap<
                                N,
                                CurrentData = Self::ParentData,
                                Output = super::#machine_ident<N>,
                            >,
                            F: FnOnce(Self::ChildMachine, Self::ChildContext) -> <N as statum::StateMarker>::Data,
                        {
                            <Self as statum::CanTransitionMap<N>>::transition_map(
                                self,
                                |child: #child_ty| f(child, ()),
                            )
                        }
                    }
                }
            }
            Self::Wrapper {
                variant_ident,
                payload_ty,
                child_ty,
                child_field_ident,
                context_struct_ident,
                context_fields,
            } => {
                let context_field_idents = context_fields
                    .iter()
                    .map(|field| field.ident.clone())
                    .collect::<Vec<_>>();
                let payload_pattern = quote! {
                    #payload_ty {
                        #child_field_ident,
                        #(#context_field_idents),*
                    }
                };
                let rebuild_payload = quote! {
                    #payload_ty {
                        #child_field_ident: f(#child_field_ident),
                        #(#context_field_idents),*
                    }
                };
                let child_context = quote! {
                    #context_struct_ident {
                        #(#context_field_idents),*
                    }
                };

                quote! {
                    impl ChildExt for super::#machine_ident<super::#variant_ident> {
                        type ChildMachine = #child_ty;
                        type ChildContext = #context_struct_ident;
                        type ParentData = #payload_ty;

                        fn child(&self) -> &Self::ChildMachine {
                            &self.state_data.#child_field_ident
                        }

                        fn map_child<F>(self, f: F) -> Self
                        where
                            F: FnOnce(Self::ChildMachine) -> Self::ChildMachine,
                        {
                            let super::#machine_ident {
                                marker,
                                state_data,
                                #(#machine_field_names),*
                            } = self;
                            let #payload_pattern = state_data;

                            super::#machine_ident {
                                marker,
                                state_data: #rebuild_payload,
                                #(#machine_field_names),*
                            }
                        }

                        fn transition_map_child<N, F>(self, f: F) -> super::#machine_ident<N>
                        where
                            N: super::#state_trait_ident + statum::StateMarker,
                            Self: statum::CanTransitionMap<
                                N,
                                CurrentData = Self::ParentData,
                                Output = super::#machine_ident<N>,
                            >,
                            F: FnOnce(Self::ChildMachine, Self::ChildContext) -> <N as statum::StateMarker>::Data,
                        {
                            <Self as statum::CanTransitionMap<N>>::transition_map(
                                self,
                                |state_data: #payload_ty| {
                                    let #payload_pattern = state_data;
                                    f(#child_field_ident, #child_context)
                                },
                            )
                        }
                    }
                }
            }
        }
    }
}

fn analyze_child_variant(
    machine_info: &MachineInfo,
    state_enum_name: &str,
    variant: &ParsedVariantInfo,
) -> Result<Option<ChildSupport>, TokenStream> {
    let variant_ident = format_ident!("{}", variant.name);
    let Some(data_ty) = variant.data_type.as_ref() else {
        return Ok(None);
    };

    if is_child_machine_type(machine_info, data_ty) {
        return Ok(Some(ChildSupport::Direct {
            variant_ident,
            child_ty: data_ty.clone(),
        }));
    }

    analyze_wrapper_payload(machine_info, state_enum_name, &variant.name, &variant_ident, data_ty)
}

fn analyze_wrapper_payload(
    machine_info: &MachineInfo,
    state_enum_name: &str,
    variant_name: &str,
    variant_ident: &Ident,
    data_ty: &Type,
) -> Result<Option<ChildSupport>, TokenStream> {
    let Some(payload_name) = plain_type_name(data_ty) else {
        return Ok(None);
    };
    let payload_is_qualified = type_path_segment_count(data_ty).is_some_and(|count| count > 1);
    let Some(file_path) = machine_info.file_path.as_deref() else {
        return Ok(None);
    };
    let Some(analysis) = get_file_analysis(file_path) else {
        return Ok(None);
    };

    let same_module_entry = analysis.structs.iter().find(|entry| {
        entry.item.ident == payload_name.clone()
            && module_path_for_line(file_path, entry.line_number).as_deref()
                == Some(machine_info.module_path.as_ref())
    });
    if let Some(entry) = same_module_entry {
        return analyze_wrapper_entry(
            machine_info,
            state_enum_name,
            variant_name,
            variant_ident,
            data_ty,
            entry,
        );
    }

    let other_module_entry = analysis.structs.iter().find(|entry| {
        entry.item.ident == payload_name.clone()
            && wrapper_entry_uses_child_machine(machine_info, entry)
            && (payload_is_qualified
                || module_path_for_line(file_path, entry.line_number)
                    .as_deref()
                    .is_some_and(|module_path| module_path != machine_info.module_path.as_ref()))
    });

    if let Some(entry) = other_module_entry {
        let module_path = module_path_for_line(file_path, entry.line_number).unwrap_or_default();
        let message = format!(
            "Error: state `{variant_name}` of `#[state]` enum `{state_enum_name}` uses wrapper payload `{payload_name}` with a nested child machine, but `{payload_name}` is declared in `{module_path}` instead of `{}`.\nFix: move `{payload_name}` into `{}`, or carry the child machine directly in `{variant_name}`.",
            machine_info.module_path,
            machine_info.module_path,
        );
        return Err(syn::Error::new_spanned(data_ty, message).to_compile_error());
    }

    Ok(None)
}

fn analyze_wrapper_entry(
    machine_info: &MachineInfo,
    state_enum_name: &str,
    variant_name: &str,
    variant_ident: &Ident,
    data_ty: &Type,
    entry: &StructEntry,
) -> Result<Option<ChildSupport>, TokenStream> {
    match &entry.item.fields {
        Fields::Named(fields) => {
            let mut child_fields = Vec::new();
            for field in &fields.named {
                if is_child_machine_field(machine_info, &field.ty) {
                    child_fields.push(field);
                }
            }

            if child_fields.is_empty() {
                return Ok(None);
            }

            if child_fields.len() > 1 {
                let payload_name = entry.item.ident.to_string();
                let child_field_names = child_fields
                    .iter()
                    .filter_map(|field| field.ident.as_ref())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                let message = format!(
                    "Error: wrapper payload `{payload_name}` for state `{variant_name}` of `#[state]` enum `{state_enum_name}` contains multiple nested child machine fields: {child_field_names}.\nFix: keep exactly one child machine field in `{payload_name}`, or carry the child machine directly in `{variant_name}`.",
                );
                return Err(syn::Error::new(entry.item.ident.span(), message).to_compile_error());
            }

            let child_field = child_fields[0];
            let child_field_ident =
                format_ident!("{}", child_field.ident.as_ref().expect("named field"));
            let child_ty = syn::parse_str::<Type>(&child_field.ty.to_token_stream().to_string())
                .map_err(|err| err.to_compile_error())?;
            let context_struct_ident = format_ident!("{}ChildContext", variant_name);
            let context_fields = fields
                .named
                .iter()
                .filter(|field| field.ident.as_ref() != Some(&child_field_ident))
                .map(|field| {
                    let ident = format_ident!("{}", field.ident.as_ref().expect("named field"));
                    let ty = syn::parse_str::<Type>(&field.ty.to_token_stream().to_string())
                        .map_err(|err| err.to_compile_error())?;
                    Ok(ContextField { ident, ty })
                })
                .collect::<Result<Vec<_>, TokenStream>>()?;

            Ok(Some(ChildSupport::Wrapper {
                variant_ident: variant_ident.clone(),
                payload_ty: data_ty.clone(),
                child_ty,
                child_field_ident,
                context_struct_ident,
                context_fields,
            }))
        }
        Fields::Unnamed(fields) => {
            if fields
                .unnamed
                .iter()
                .any(|field| is_child_machine_field(machine_info, &field.ty))
            {
                let payload_name = entry.item.ident.to_string();
                let message = format!(
                    "Error: wrapper payload `{payload_name}` for state `{variant_name}` of `#[state]` enum `{state_enum_name}` must use named fields when carrying a nested child machine.\nFix: change `{payload_name}` to a named-field struct and keep exactly one child machine field."
                );
                return Err(syn::Error::new_spanned(data_ty, message).to_compile_error());
            }
            Ok(None)
        }
        Fields::Unit => Ok(None),
    }
}

fn wrapper_entry_uses_child_machine(machine_info: &MachineInfo, entry: &StructEntry) -> bool {
    match &entry.item.fields {
        Fields::Named(fields) => fields
            .named
            .iter()
            .any(|field| is_child_machine_field(machine_info, &field.ty)),
        Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .any(|field| is_child_machine_field(machine_info, &field.ty)),
        Fields::Unit => false,
    }
}

fn plain_type_name(ty: &Type) -> Option<&Ident> {
    let Type::Path(TypePath { qself: None, path }) = ty else {
        return None;
    };
    let segment = path.segments.last()?;
    if !matches!(segment.arguments, syn::PathArguments::None) {
        return None;
    }
    Some(&segment.ident)
}

fn type_path_segment_count(ty: &Type) -> Option<usize> {
    let Type::Path(TypePath { qself: None, path }) = ty else {
        return None;
    };
    Some(path.segments.len())
}

fn is_child_machine_type(machine_info: &MachineInfo, ty: &Type) -> bool {
    let Some((machine_name, _state_name)) = machine_signature(ty) else {
        return false;
    };

    machine_name_known_in_file(machine_info, &machine_name)
        || machine_name == "Machine"
        || machine_name.ends_with("Machine")
}

fn is_child_machine_field(machine_info: &MachineInfo, ty: &Type) -> bool {
    if is_child_machine_type(machine_info, ty) {
        return true;
    }

    let tokens = ty.to_token_stream().to_string();
    tokens.contains("Machine <") || tokens.contains("Machine<")
}

fn machine_name_known_in_file(machine_info: &MachineInfo, machine_name: &str) -> bool {
    let Some(file_path) = machine_info.file_path.as_deref() else {
        return false;
    };
    let Some(analysis) = get_file_analysis(file_path) else {
        return false;
    };

    analysis.structs.iter().any(|entry| {
        entry.item.ident == machine_name && entry.attrs.iter().any(|attr| attr == "machine")
    })
}

fn machine_signature(ty: &Type) -> Option<(String, String)> {
    let Type::Path(TypePath { qself: None, path }) = ty else {
        return None;
    };
    let segment = path.segments.last()?;
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    if args.args.len() != 1 {
        return None;
    }
    let syn::GenericArgument::Type(Type::Path(state_ty)) = args.args.first()? else {
        return None;
    };
    let state_name = state_ty.path.segments.last()?.ident.to_string();
    Some((segment.ident.to_string(), state_name))
}
