moddef::moddef!(
    flat (pub) mod {
    },
    flat (pub(crate)) mod {
        state,
        machine,
        transition
    }
);

use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{parse_macro_input, Block, Expr, ExprBlock, FnArg, ItemEnum, ItemImpl, ItemStruct};
use syn::{Ident, Type};

#[proc_macro_attribute]
pub fn state(_attr: TokenStream, item: TokenStream) -> TokenStream {
    println!("[state] Starting macro execution...");

    let input = parse_macro_input!(item as ItemEnum);

    // Validate the enum before proceeding
    if let Some(error) = validate_state_enum(&input) {
        return error.into();
    }

    let enum_info = EnumInfo::from_item_enum(&input).expect("Failed to parse EnumInfo");

    println!("[state] Parsed Enum: {}", enum_info.name);

    // Store metadata in `state_enum_map`
    store_state_enum(&enum_info);

    // Generate structs and implementations dynamically
    let expanded = generate_state_impls(&enum_info.file_path);

    println!("[state] Macro execution completed.");
    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn machine(_attr: TokenStream, item: TokenStream) -> TokenStream {
    println!("[machine] Starting macro execution...");

    let input = parse_macro_input!(item as ItemStruct);

    let machine_info = MachineInfo::from_item_struct(&input).expect("Failed to parse MachineInfo");

    // Validate the struct before proceeding
    if let Some(error) = validate_machine_struct(&input, &machine_info) {
        return error.into();
    }

    println!("[machine] Parsed Struct: {}", machine_info.name);

    // Store metadata in `machine_map`
    store_machine_struct(&machine_info);

    // Generate any required structs or implementations dynamically
    let expanded = generate_machine_impls(&machine_info);

    println!("[machine] Macro execution completed.");
    TokenStream::from(expanded)
}

// Struct to represent a function inside an impl block
#[derive(Debug)]
struct TransitionFn {
    name: Ident,
    args: Vec<String>,
    return_type: Option<Type>,
    generics: Vec<Ident>,
    internals: Block,
}

// Struct to represent the entire transition impl block
#[derive(Debug)]
struct TransitionImpl {
    target_type: Type, // e.g., Machine<Draft>
    functions: Vec<TransitionFn>,
}

#[proc_macro_attribute]
pub fn transition(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as ItemImpl);

    let file_path = std::env::current_dir()
        .expect("Failed to get current directory.")
        .to_string_lossy()
        .to_string();

    // Extract the target type from the impl block
    let target_type = *input.self_ty.clone();

    // Validate that the target type consists of a valid Machine and State
    let machine_map = get_machine_map().read().unwrap();
    let state_enum_map = get_state_enum_map().read().unwrap();

    let target_type_str = target_type.to_token_stream().to_string();
    let mut type_parts = target_type_str.split('<');
    let machine_name = type_parts.next().unwrap_or("").trim().to_string();
    let state_name = type_parts
        .next()
        .map(|s| s.trim_end_matches('>').trim().to_string());

    let machine_info = machine_map.get(&file_path.clone().into());

    let state_enum_info = state_enum_map.iter().find(|(state_enum_file_path, info)| {
        let state_enum_file_path = state_enum_file_path.as_ref().to_owned();
        state_name.as_ref().is_some_and(|state| {
            state_enum_file_path == file_path
                && info.variants.iter().any(|variant| &variant.name == state)
        })
    });

    if state_enum_info.is_none() {
        return quote! {
            compile_error!(concat!("Invalid state variant: ", #state_name, " is not a valid variant of a registered #[state] enum."));
        }.into();
    }

    let machine_name_clone = machine_name.clone();

    if machine_info.is_none() {
        return quote! {
            compile_error!(concat!("Invalid machine: ", #machine_name_clone, " is not present in this file. Did you forget to add a #[machine] attribute?"));
        }.into();
    }

    let machine_info = machine_info.unwrap();
    let state_enum_info = state_enum_info.unwrap().1;

    // Extract function data
    let mut functions = Vec::new();
    for item in input.clone().items {
        if let syn::ImplItem::Fn(method) = item {
            let name = method.sig.ident.clone();
            let args: Vec<String> = method
                .sig
                .inputs
                .iter()
                .map(|arg| match arg {
                    FnArg::Receiver(_) => "self".to_string(),
                    FnArg::Typed(pat_type) => pat_type.ty.to_token_stream().to_string(),
                })
                .collect();

            let return_type = if let syn::ReturnType::Type(_, ty) = &method.sig.output {
                Some(*ty.clone())
            } else {
                None
            };

            let generics = method
                .sig
                .generics
                .params
                .iter()
                .filter_map(|param| {
                    if let syn::GenericParam::Type(type_param) = param {
                        Some(type_param.ident.clone())
                    } else {
                        None
                    }
                })
                .collect();

            let internals = method.block.clone(); // Directly clone the block

            functions.push(TransitionFn {
                name,
                args,
                return_type,
                generics,
                internals,
            });
        }
    }

    if functions.is_empty() {
        return quote! {
            compile_error!("#[transition] impl blocks must contain at least one method returning Machine<SomeState>.");
        }.into();
    }

    // Ensure all transition functions have exactly one argument (self)
    for function in &functions {
        if function.args.len() != 1 || function.args[0] != "self" {
            let func_name = &function.name;
            return quote! {
                compile_error!(concat!("Invalid function signature: ", stringify!(#func_name), " transition functions must only take 'self' as an argument."));
            }.into();
        }
    }

    let state_trait = state_enum_info.get_trait_name();

    let machine_name_ident = format_ident!("{}", machine_info.name);

    let this_state = state_enum_info
        .variants
        .iter()
        .find(|v| &v.name == state_name.as_ref().unwrap())
        .expect("Failed to find state variant.");

    let this_states_data_type = format_ident!(
        "{}",
        this_state
            .data_type
            .clone()
            .expect("Failed to find data type for state variant.")
    );

    let fields = machine_info.fields_to_token_stream();

    let transition_impl = if this_state.data_type.is_none() {
        quote! {
            pub fn transition<NewState: #state_trait>(self) -> #machine_name_ident<NewState>
            where
                NewState: #state_trait<Data = ()>
            {
                #machine_name_ident {
                    #fields
                    marker: core::marker::PhantomData,
                    state_data: None,
                }
            }
        }
    } else {
        quote! {
            pub fn transition<NewState: #state_trait>(self, data: NewState::Data) -> #machine_name_ident<NewState>
            where
                NewState: #state_trait<Data = #this_states_data_type>
            {
                #machine_name_ident {
                    #fields
                    marker: core::marker::PhantomData,
                    state_data: data,
                }
            }
        }
    };

    let function_tokens = functions.iter().map(|function| {
        let name = &function.name;
        let args = function.args.iter().map(|arg| format_ident!("{}", arg));
        let generics = function.generics.iter().map(|gen| format_ident!("{}", gen));
        let return_type = function.return_type.as_ref().map(|ty| quote! { -> #ty });
        let internals_token_stream = &function.internals; // Directly use syn::Block

        quote! {
            pub fn #name<#(#generics),*>(#(#args),*) #return_type
                #internals_token_stream
        }
    });

    // Generate output (for now, just return the original input unchanged)
    let output = quote! {

        impl #target_type {
            #transition_impl
            #(#function_tokens)*
        }
    };

    output.into()
}
