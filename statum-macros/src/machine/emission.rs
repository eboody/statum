use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{GenericParam, Generics, Ident, ItemStruct, Visibility};

use crate::state::{ParsedEnumInfo, ParsedVariantInfo};
use crate::{EnumInfo, to_snake_case};

use super::metadata::{ParsedMachineInfo, field_type_alias_name, is_rust_analyzer};
use super::registry::get_machine_map;
use super::MachineInfo;

pub fn generate_machine_impls(machine_info: &MachineInfo, item: &ItemStruct) -> proc_macro2::TokenStream {
    let map_guard = match get_machine_map().read() {
        Ok(guard) => guard,
        Err(_) => {
            let message = format!(
                "Internal error: machine metadata lock poisoned while generating `{}` in module `{}`.",
                machine_info.name, machine_info.module_path.0
            );
            return quote! {
                compile_error!(#message);
            };
        }
    };
    let Some(machine_info) = map_guard.get(&machine_info.module_path) else {
        let message = format!(
            "Internal error: machine metadata for `{}` in module `{}` was not cached during code generation.\nTry re-running `cargo check` and make sure `#[machine]` is applied in that module.",
            machine_info.name, machine_info.module_path.0
        );
        return quote! {
            compile_error!(#message);
        };
    };

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
    let transition_support = transition_support(machine_info, &state_enum);
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

    quote! {
        #transition_support
        #field_type_aliases
        #struct_def
        #builder_methods
        #machine_state_surface
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

    let default_state_ident = format_ident!("Uninitialized{}", state_enum.name);
    first_type.default = Some(syn::parse_quote!(#default_state_ident));
    first_type.eq_token = Some(syn::Token![=](Span::call_site()));

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

fn transition_support(machine_info: &MachineInfo, state_enum: &EnumInfo) -> TokenStream {
    let trait_name = state_enum.get_trait_name();
    let machine_ident = format_ident!("{}", machine_info.name);
    let support_module_ident = transition_support_module_ident(machine_info);
    quote! {
        #[doc(hidden)]
        mod #support_module_ident {
            use super::*;

            pub trait TransitionTo<N: #trait_name> {
                fn transition(self) -> #machine_ident<N>;
            }

            pub trait TransitionWith<T> {
                type NextState: #trait_name;
                fn transition_with(self, data: T) -> #machine_ident<Self::NextState>;
            }
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
    let fields_struct_fields = parsed_machine.fields.iter().map(|field| {
        let field_ident = &field.ident;
        let alias_ident = format_ident!(
            "{}",
            field_type_alias_name(&machine_info.name, &field.ident.to_string())
        );
        quote! {
            pub #field_ident: super::#alias_ident
        }
    });
    let state_variants = parsed_state.variants.iter().map(|variant| {
        let variant_ident = format_ident!("{}", variant.name);
        quote! {
            #variant_ident(super::#machine_ident<super::#variant_ident>)
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
    let module_ident = format_ident!("{}", to_snake_case(&machine_info.name));

    Ok(quote! {
        #vis mod #module_ident {
            pub struct Fields {
                #(#fields_struct_fields),*
            }

            pub enum State {
                #(#state_variants),*
            }

            pub trait IntoMachinesExt<Item>: Sized {
                type Builder;
                type BuilderWithFields<F>;

                fn into_machines(self) -> Self::Builder;

                fn into_machines_by<F>(self, fields: F) -> Self::BuilderWithFields<F>
                where
                    F: Fn(&Item) -> Fields;
            }

            impl State {
                #(#is_methods)*
            }
        }
    })
}

fn generate_field_type_aliases(machine_info: &MachineInfo, item: &ItemStruct) -> TokenStream {
    let alias_vis = &item.vis;
    let aliases = item.fields.iter().filter_map(|field| {
        let field_ident = field.ident.as_ref()?;
        let alias_ident =
            format_ident!("{}", field_type_alias_name(&machine_info.name, &field_ident.to_string()));
        let field_ty = &field.ty;
        Some(quote! {
            #[doc(hidden)]
            #[allow(non_camel_case_types)]
            #alias_vis type #alias_ident = #field_ty;
        })
    });

    quote! {
        #(#aliases)*
    }
}

pub(crate) fn transition_support_module_ident(machine_info: &MachineInfo) -> Ident {
    format_ident!(
        "__statum_{}_transition",
        to_snake_case(&machine_info.name)
    )
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

    Ok(quote! {
        #derives
        #vis struct #machine_ident #generics {
            marker: core::marker::PhantomData<#state_generic_ident>,
            pub state_data: #state_generic_ident::Data,
            #( #field_tokens ),*
        }

        impl #impl_generics #machine_ident #ty_generics #where_clause {
            #vis fn transition_map<N, F>(self, f: F) -> #machine_ident<N>
            where
                N: #state_trait_ident + statum::StateMarker,
                Self: statum::CanTransitionMap<N, Output = #machine_ident<N>>,
                F: FnOnce(<Self as statum::CanTransitionMap<N>>::CurrentData) -> <N as statum::StateMarker>::Data,
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
    generate_custom_builder_tokens(context, &variant_ident, &variant_builder_ident, variant.data_type.as_ref())
}

fn generate_custom_builder_tokens(
    context: &BuilderContext<'_>,
    variant_ident: &Ident,
    variant_builder_ident: &Ident,
    data_type: Option<&syn::Type>,
) -> TokenStream {
    let machine_ident = context.machine_ident;
    let builder_vis = context.builder_vis;
    let field_names = context.field_names;
    let field_types = context.field_types;
    let struct_initialization = machine_struct_initialization(context, data_type.is_some());

    if context.use_ra_shim {
        let state_data_method = data_type.map(|parsed_data_type| {
            quote! {
                #builder_vis fn state_data(self, _data: #parsed_data_type) -> Self {
                    self
                }
            }
        });

        return quote! {
            #builder_vis struct #variant_builder_ident;

            impl #variant_builder_ident {
                #state_data_method
                #(#builder_vis fn #field_names(self, _value: #field_types) -> Self { self })*

                #builder_vis fn build(self) -> #machine_ident<#variant_ident> {
                    panic!("statum rust-analyzer shim: builder values are not constructed at runtime")
                }
            }

            impl #machine_ident<#variant_ident> {
                #builder_vis fn builder() -> #variant_builder_ident {
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
    let builder_init = slot_storage_idents.iter().map(|storage_ident| {
        quote! { #storage_ident: core::option::Option::None }
    });
    let complete_builder_ty_generics = if slot_state_idents.is_empty() {
        quote! {}
    } else {
        let complete = slot_state_idents.iter().map(|_| quote! { true });
        quote! { <#(#complete),*> }
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

        impl #machine_ident<#variant_ident> {
            #builder_vis fn builder() -> #variant_builder_ident {
                #variant_builder_ident {
                    #(#builder_init),*
                }
            }
        }

        impl #builder_impl_generics #variant_builder_ident #builder_ty_generics {
            #(#setters)*
        }

        impl #variant_builder_ident #complete_builder_ty_generics {
            #builder_vis fn build(self) -> #machine_ident<#variant_ident> {
                #state_data_binding
                #(#field_bindings)*
                #struct_initialization
            }
        }
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
