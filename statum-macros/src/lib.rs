use proc_macro::TokenStream;
use quote::format_ident;
use quote::quote;
use quote::ToTokens;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::Ident;
use syn::{
    parse::Parser, parse_macro_input, punctuated::Punctuated, Data, DeriveInput, Fields, ItemImpl,
    Path, PathArguments, ReturnType, Token, Type,
};

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;

#[derive(Clone, Debug)]
struct VariantInfo {
    name: String,
    data_type: Option<String>, // e.g., "DraftData" for InProgress, None for unit variants
}

static STATE_VARIANTS: OnceLock<Mutex<HashMap<String, Vec<VariantInfo>>>> = OnceLock::new();

fn get_variants_map() -> &'static Mutex<HashMap<String, Vec<VariantInfo>>> {
    STATE_VARIANTS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn register_state_variants(enum_name: String, variants: Vec<VariantInfo>) {
    let map = get_variants_map();
    map.lock().unwrap().insert(enum_name, variants);
}

pub(crate) fn get_state_variants(enum_name: &str) -> Option<Vec<VariantInfo>> {
    let map = get_variants_map();
    map.lock().unwrap().get(enum_name).cloned()
}

static MACHINE_FIELDS: OnceLock<Mutex<HashMap<String, Vec<(String, String)>>>> = OnceLock::new();

// Helper to get or init the storage
fn get_fields_map() -> &'static Mutex<HashMap<String, Vec<(String, String)>>> {
    MACHINE_FIELDS.get_or_init(|| Mutex::new(HashMap::new()))
}

// Helper to register fields from #[machine]
pub(crate) fn register_machine_fields(enum_name: String, fields: Vec<(String, String)>) {
    let map = get_fields_map();
    map.lock().unwrap().insert(enum_name, fields);
}

// Helper to get fields for #[model]
pub(crate) fn get_machine_fields(enum_name: &str) -> Option<Vec<(String, String)>> {
    let map = get_fields_map();
    map.lock().unwrap().get(enum_name).cloned()
}

#[derive(Clone)]
struct ModelAttr {
    machine: syn::Path,
    state: syn::Path,
}

#[derive(Clone, Debug)]
struct ValidatorsAttr {
    state: Ident,
    machine: Ident,
}

impl Parse for ValidatorsAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Ident>()?; // parse "state"
        input.parse::<Token![=]>()?;
        let state = input.parse()?;
        input.parse::<Token![,]>()?;
        input.parse::<Ident>()?; // parse "machine"
        input.parse::<Token![=]>()?;
        let machine = input.parse()?;
        Ok(ValidatorsAttr { state, machine })
    }
}

impl syn::parse::Parse for ModelAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut machine = None;
        let mut state = None;

        // Parse first pair
        let name1: syn::Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        let value1: syn::Path = input.parse()?;

        // Store in correct field
        match name1.to_string().as_str() {
            "machine" => machine = Some(value1),
            "state" => state = Some(value1),
            _ => {
                return Err(syn::Error::new(
                    name1.span(),
                    "Expected 'machine' or 'state'",
                ))
            }
        }

        // Parse comma
        input.parse::<Token![,]>()?;

        // Parse second pair
        let name2: syn::Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        let value2: syn::Path = input.parse()?;

        // Store in correct field
        match name2.to_string().as_str() {
            "machine" => {
                if machine.is_some() {
                    return Err(syn::Error::new(
                        name2.span(),
                        "Duplicate 'machine' parameter",
                    ));
                }
                machine = Some(value2);
            }
            "state" => {
                if state.is_some() {
                    return Err(syn::Error::new(name2.span(), "Duplicate 'state' parameter"));
                }
                state = Some(value2);
            }
            _ => {
                return Err(syn::Error::new(
                    name2.span(),
                    "Expected 'machine' or 'state'",
                ))
            }
        }

        // Ensure we got both parameters
        match (machine, state) {
            (Some(machine), Some(state)) => Ok(ModelAttr { machine, state }),
            _ => Err(syn::Error::new(
                name1.span(),
                "Must specify both 'machine' and 'state'",
            )),
        }
    }
}

fn get_field_info(input: &DeriveInput) -> (Vec<&syn::Ident>, Vec<&syn::Type>) {
    match &input.data {
        syn::Data::Struct(s) => match &s.fields {
            syn::Fields::Named(fields) => {
                let param_names = fields
                    .named
                    .iter()
                    .filter(|f| {
                        f.ident
                            .as_ref()
                            .is_some_and(|i| i != "marker" && i != "state_data")
                    })
                    .map(|f| f.ident.as_ref().unwrap())
                    .collect::<Vec<_>>();
                let param_types = fields
                    .named
                    .iter()
                    .filter(|f| {
                        f.ident
                            .as_ref()
                            .is_some_and(|i| i != "marker" && i != "state_data")
                    })
                    .map(|f| &f.ty)
                    .collect::<Vec<_>>();
                (param_names, param_types)
            }
            _ => panic!("Only named fields are supported"),
        },
        _ => panic!("Only structs are supported"),
    }
}

#[proc_macro_attribute]
pub fn state(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let vis = &input.vis;
    let name = &input.ident;

    // Extract variants with data type info
    let variants: Vec<VariantInfo> = match &input.data {
        Data::Enum(data_enum) => data_enum
            .variants
            .iter()
            .map(|v| {
                let name = v.ident.to_string();
                let data_type = match &v.fields {
                    Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                        // Single-field tuple variant
                        Some(
                            fields
                                .unnamed
                                .first()
                                .unwrap()
                                .ty
                                .to_token_stream()
                                .to_string(),
                        )
                    }
                    Fields::Unit => None,
                    _ => None, // For simplicity; handle other cases as needed
                };
                VariantInfo { name, data_type }
            })
            .collect(),
        _ => panic!("#[state] can only be used on enums"),
    };

    // Register variants with their data types
    register_state_variants(name.to_string(), variants);

    // Analyze user-supplied #[derive(...)] to detect which traits they want
    #[allow(unused_variables)]
    let (
        user_derives,
        wants_serialize,
        wants_deserialize,
        wants_debug,
        wants_clone,
        wants_default,
        wants_eq,
        wants_partial_eq,
        wants_hash,
        wants_partial_ord,
        wants_ord,
        wants_copy,
    ) = analyze_user_derives(&input.attrs);

    // We'll accumulate any trait bounds we need in "trait_bounds".
    let mut trait_bounds = vec![];

    // If the user derived Debug, we add std::fmt::Debug as a bound.
    if wants_debug {
        trait_bounds.push(quote!(std::fmt::Debug));
    }

    // Only add serde bounds if our crate's "serde" feature is enabled.
    #[cfg(feature = "serde")]
    {
        if wants_serialize {
            trait_bounds.push(quote!(serde::Serialize));
        }
        if wants_deserialize {
            trait_bounds.push(quote!(serde::de::DeserializeOwned));
        }
    }

    let trait_bounds = if trait_bounds.is_empty() {
        quote!()
    } else {
        quote!(: #(#trait_bounds +)*)
    };

    // We'll replicate all user-specified derives on each generated variant struct.
    let replicate_derives = if user_derives.is_empty() {
        quote!()
    } else {
        quote! {
            #[derive(#(#user_derives),*)]
        }
    };

    // Convert each enum variant into a separate struct with an impl that ties back to the "State" trait.
    let states = match &input.data {
        Data::Enum(data_enum) => data_enum.variants.iter().map(|variant| {
            let variant_ident = &variant.ident;
            let variant_fields = &variant.fields;
            match variant_fields {
                // Single-field tuple variant
                Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                    let field_type = &fields.unnamed.first().unwrap().ty;
                    quote! {
                        #replicate_derives
                        #vis struct #variant_ident(#field_type);

                        impl #name for #variant_ident {
                            type Data = #field_type;
                            const HAS_DATA: bool = true;

                            fn get_data(&self) -> Option<&Self::Data> {
                                Some(&self.0)
                            }

                            fn get_data_mut(&mut self) -> Option<&mut Self::Data> {
                                Some(&mut self.0)
                            }
                        }
                    }
                }
                // Unit variant
                Fields::Unit => {
                    quote! {
                        #replicate_derives
                        #vis struct #variant_ident;

                        impl #name for #variant_ident {
                            type Data = ();
                            const HAS_DATA: bool = false;

                            fn get_data(&self) -> Option<&Self::Data> {
                                None
                            }

                            fn get_data_mut(&mut self) -> Option<&mut Self::Data> {
                                None
                            }
                        }
                    }
                }
                _ => panic!("Variants must be unit or single-field tuple variants"),
            }
        }),
        _ => {
            return syn::Error::new_spanned(&input.ident, "#[state] can only be used on an enum")
                .to_compile_error()
                .into();
        }
    };

    let expanded = quote! {
        // The trait for this "state" enum
        #vis trait #name {
            type Data #trait_bounds;
            const HAS_DATA: bool;
            fn get_data(&self) -> Option<&Self::Data>;
            fn get_data_mut(&mut self) -> Option<&mut Self::Data>;
        }

        // One struct + impl per variant
        #(#states)*
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn machine(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as DeriveInput);
    let struct_name = &input.ident;
    let state_trait = extract_state_trait(&input);

    // Insert "marker" and "state_data" fields into the user's struct.
    if let syn::Data::Struct(ref mut struct_data) = input.data {
        if let syn::Fields::Named(ref mut fields) = struct_data.fields {
            fields.named.push(
                syn::Field::parse_named
                    .parse2(quote! { marker: core::marker::PhantomData<S> })
                    .unwrap(),
            );
            fields.named.push(
                syn::Field::parse_named
                    .parse2(quote! { state_data: Option<S::Data> })
                    .unwrap(),
            );
        }
    }

    let (field_names, field_types) = get_field_info(&input);

    // Register field names and their types
    let fields_with_types: Vec<(String, String)> = field_names
        .iter()
        .zip(field_types.iter())
        .map(|(name, ty)| (name.to_string(), ty.to_token_stream().to_string()))
        .collect();

    register_machine_fields(struct_name.to_string(), fields_with_types);

    let transition_impl = quote! {
        impl<CurrentState: #state_trait> #struct_name<CurrentState> {
            pub fn transition<NewState: #state_trait>(self) -> #struct_name<NewState>
            where
                NewState: #state_trait<Data = ()>
            {
                #struct_name {
                    #(#field_names: self.#field_names,)*
                    marker: core::marker::PhantomData,
                    state_data: None,
                }
            }

            pub fn transition_with<NewState: #state_trait>(self, data: NewState::Data) -> #struct_name<NewState> {
                #struct_name {
                    #(#field_names: self.#field_names,)*
                    marker: core::marker::PhantomData,
                    state_data: Some(data),
                }
            }

            pub fn get_state_data(&self) -> Option<&CurrentState::Data> {
                self.state_data.as_ref()
            }

            pub fn get_state_data_mut(&mut self) -> Option<&mut CurrentState::Data> {
                self.state_data.as_mut()
            }
        }
    };

    let constructor = quote! {
        impl<S: #state_trait> #struct_name<S> {
            pub fn new(#(#field_names: #field_types),*) -> Self {
                Self {
                    #(#field_names,)*
                    marker: core::marker::PhantomData,
                    state_data: None,
                }
            }
        }
    };

    let expanded = quote! {
        #input
        #transition_impl
        #constructor
    };

    TokenStream::from(expanded)
}

fn extract_state_trait(input: &DeriveInput) -> syn::Ident {
    let generics = &input.generics;
    let type_param = generics
        .type_params()
        .next()
        .expect("Struct must have a type parameter");
    let bounds = &type_param.bounds;
    for bound in bounds {
        if let syn::TypeParamBound::Trait(trait_bound) = bound {
            if let Some(segment) = trait_bound.path.segments.last() {
                return segment.ident.clone();
            }
        }
    }
    panic!("Type parameter must have a trait bound");
}

fn analyze_user_derives(
    attrs: &[syn::Attribute],
) -> (
    Vec<Path>,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
) {
    let mut user_derives = Vec::new();
    let mut wants_serialize = false;
    let mut wants_deserialize = false;
    let mut wants_debug = false;
    let mut wants_clone = false;
    let mut wants_default = false;
    let mut wants_eq = false;
    let mut wants_partial_eq = false;
    let mut wants_hash = false;
    let mut wants_partial_ord = false;
    let mut wants_ord = false;
    let mut wants_copy = false;

    // Parse `#[derive(...)]` with syn 2.0
    for attr in attrs {
        if attr.path().is_ident("derive") {
            if let Ok(paths) = attr.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)
            {
                for path in paths {
                    if let Some(ident) = path.get_ident() {
                        match ident.to_string().as_str() {
                            "Serialize" => wants_serialize = true,
                            "Deserialize" => wants_deserialize = true,
                            "Debug" => wants_debug = true,
                            "Clone" => wants_clone = true,
                            "Default" => wants_default = true,
                            "Eq" => wants_eq = true,
                            "PartialEq" => wants_partial_eq = true,
                            "Hash" => wants_hash = true,
                            "PartialOrd" => wants_partial_ord = true,
                            "Ord" => wants_ord = true,
                            "Copy" => wants_copy = true,
                            _ => {}
                        }
                    }
                    user_derives.push(path);
                }
            }
        }
    }

    (
        user_derives,
        wants_serialize,
        wants_deserialize,
        wants_debug,
        wants_clone,
        wants_default,
        wants_eq,
        wants_partial_eq,
        wants_hash,
        wants_partial_ord,
        wants_ord,
        wants_copy,
    )
}

#[proc_macro_attribute]
pub fn model(attr: TokenStream, item: TokenStream) -> TokenStream {
    let ModelAttr { machine, state } = parse_macro_input!(attr as ModelAttr);
    let input = parse_macro_input!(item as DeriveInput);
    let struct_name = &input.ident;

    // Use reference to machine when converting to token stream
    let machine_input = syn::parse_str::<DeriveInput>(&machine.to_token_stream().to_string())
        .expect("Could not parse machine type");

    let (field_names, field_types) = get_field_info(&machine_input);

    let state_name = state
        .get_ident()
        .expect("Expected simple state name")
        .to_string();

    let variants = get_state_variants(&state_name)
        .expect("State type not found - did you mark it with #[state]?");

    // Generate try_to_* methods using the actual fields
    let try_methods = variants.iter().map(|variant| {
        let variant_ident = format_ident!("{}", variant.name);
        let try_method_name = format_ident!("try_to_{}", to_snake_case(&variant.name));
        let is_method_name = format_ident!("is_{}", to_snake_case(&variant.name));

        quote! {
            pub fn #try_method_name(&self, #(#field_names: #field_types),*) -> Result<#machine<#variant_ident>, statum::Error> {
                if self.#is_method_name() {
                    Ok(#machine::<#variant_ident>::new(#(#field_names),*))
                } else {
                    Err(statum::Error::InvalidState)
                }
            }
        }
    });

    let expanded = quote! {
        #input

        impl #struct_name {
            #(#try_methods)*
        }
    };

    TokenStream::from(expanded)
}

// Helper function to convert PascalCase to snake_case
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && c.is_uppercase() {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap());
    }
    result
}

/// The user would do:
/// ```
/// #[validators(state = TaskState, machine = TaskMachine)]
/// impl DbData {
///     fn is_new(&self /*, ..TaskMachine fields*/) -> Result<(), statum::Error> { ... }
///     fn is_in_progress(&self /*, ..TaskMachine fields*/) -> Result<DraftData, statum::Error> { ... }
///     fn is_complete(&self /*, ..TaskMachine fields*/) -> Result<(), statum::Error> { ... }
/// }
/// ```
/// We parse the `impl` block, find all `is_*` fns, build a wrapper enum,
/// and build a `to_machine(/*..TaskMachine fields*/)` method.
#[proc_macro_attribute]
pub fn validators(attr: TokenStream, item: TokenStream) -> TokenStream {
    // 1. Parse the attribute: #[validators(state = ..., machine = ...)]
    let (state_ident, machine_ident) = match parse_validators_attr(attr) {
        Ok(pair) => pair,
        Err(e) => return e.to_compile_error().into(),
    };

    // 2. Parse the `impl` block itself
    let impl_block = parse_macro_input!(item as ItemImpl);

    // Get the type of the `impl`
    let self_ty = &impl_block.self_ty;

    // Get the state variants
    let enum_variants = match get_variants_of_state(&state_ident) {
        Ok(vars) => vars,
        Err(e) => return e.to_compile_error().into(),
    };

    // Generate the wrapper enum
    let wrapper_enum_ident = format_ident!("{}State", machine_ident);
    let wrapper_variants = enum_variants.iter().map(|variant| {
        let v_id = format_ident!("{}", variant.name);
        quote! {
            #v_id(#machine_ident<#v_id>)
        }
    });

    let is_methods = enum_variants.iter().map(|variant| {
        let variant_ident = format_ident!("{}", variant.name);
        let method_name = format_ident!("is_{}", to_snake_case(&variant.name));

        quote! {
            pub fn #method_name(&self) -> bool {
                matches!(self, Self::#variant_ident(_))
            }
        }
    });

    let wrapper_variants_match_arms = enum_variants.iter().map(|variant| {
        let variant_ident = format_ident!("{}", variant.name);

        quote! {
            Self::#variant_ident(machine) => Some(machine)
        }
    });

    let wrapper_enum = quote! {
        pub enum #wrapper_enum_ident {
            #(#wrapper_variants),*
        }

        impl #wrapper_enum_ident {
            #(#is_methods)*

            pub fn as_ref(&self) -> Option<&dyn std::any::Any> {
                match self {
                    #(#wrapper_variants_match_arms),*
                }
            }
        }
    };

    // Get machine field names and types
    let machine_name_str = machine_ident.to_string();
    let field_names_opt = get_machine_fields(&machine_name_str);
    if field_names_opt.is_none() {
        return syn::Error::new_spanned(
            machine_ident,
            format!(
                "Machine '{}' does not have registered fields",
                machine_name_str
            ),
        )
        .to_compile_error()
        .into();
    }
    let fields = field_names_opt.unwrap();

    let is_fns: Vec<&syn::ImplItemFn> = impl_block
        .items
        .iter()
        .filter_map(|item| {
            if let syn::ImplItem::Fn(func) = item {
                Some(func)
            } else {
                None
            }
        })
        .collect();

    // Generate `to_machine` function
    let (to_machine_checks, has_async) =
        build_to_machine_fn(&enum_variants, &is_fns, &machine_ident, &wrapper_enum_ident);

    // Generate field identifiers and types
    let field_idents = fields
        .iter()
        .map(|(name, _type)| format_ident!("{}", name))
        .collect::<Vec<_>>();

    let field_types = fields
        .iter()
        .map(|(_name, ty)| syn::parse_str::<syn::Type>(ty).unwrap())
        .collect::<Vec<_>>();

    let to_machine_signature = if has_async {
        quote! {
            pub async fn to_machine(&self, #( #field_idents: #field_types ),* ) -> core::result::Result<#wrapper_enum_ident, statum::Error>
        }
    } else {
        quote! {
            pub fn to_machine(&self, #( #field_idents: #field_types ),* ) -> core::result::Result<#wrapper_enum_ident, statum::Error>
        }
    };

    let try_methods = enum_variants.iter().map(|variant| {
        let variant_ident = format_ident!("{}", variant.name);
        let try_method_name = format_ident!("try_to_{}", to_snake_case(&variant.name));
        let is_method_name = format_ident!("is_{}", to_snake_case(&variant.name));

        // Check if the `is_*` function is async
        let is_async = is_fns.iter().any(|func| func.sig.ident == is_method_name && func.sig.asyncness.is_some());

        if is_async {
            // Generate an async method
            quote! {
                pub async fn #try_method_name(&self, #(#field_idents: #field_types),*) -> core::result::Result<#machine_ident<#variant_ident>, statum::Error> {
                    if self.#is_method_name(#(&#field_idents),*).await.is_ok() {
                        Ok(#machine_ident::<#variant_ident>::new(#(#field_idents),*))
                    } else {
                        Err(statum::Error::InvalidState)
                    }
                }
            }
        } else {
            // Generate a sync method
            quote! {
                pub fn #try_method_name(&self, #(#field_idents: #field_types),*) -> core::result::Result<#machine_ident<#variant_ident>, statum::Error> {
                    if self.#is_method_name(#(&#field_idents),*).is_ok() {
                        Ok(#machine_ident::<#variant_ident>::new(#(#field_idents),*))
                    } else {
                        Err(statum::Error::InvalidState)
                    }
                }
            }
        }
    });

    let modified_impl_items: Vec<proc_macro2::TokenStream> = impl_block
        .items
        .iter()
        .map(|item| {
            if let syn::ImplItem::Fn(method) = item {
                // Check if this method needs modification (e.g., starts with "is_")
                if method.sig.ident.to_string().starts_with("is_") {
                    let sig = &method.sig;
                    let method_name = &sig.ident;

                    // Rebuild method signature with additional parameters
                    let mut updated_inputs: Vec<syn::FnArg> = sig.inputs.iter().cloned().collect();
                    for (field_name, field_type) in &fields {
                        let field_ident =
                            syn::parse_str::<syn::Ident>(field_name).unwrap_or_else(|_| {
                                panic!("Failed to parse '{}' as Ident", field_name)
                            });

                        let field_ty = if field_type == "String" {
                            syn::parse_str::<syn::Type>("str").unwrap()
                        } else {
                            syn::parse_str::<syn::Type>(field_type).unwrap()
                        };

                        updated_inputs.push(syn::FnArg::Typed(syn::parse_quote! {
                            #field_ident: &#field_ty
                        }));
                    }

                    let method_body = &method.block;
                    let asyncness = &sig.asyncness;
                    let output = &sig.output;

                    // Generate the modified method
                    quote! {
                        pub #asyncness fn #method_name(#(#updated_inputs),*) #output {
                            #method_body
                        }
                    }
                } else {
                    // Keep other methods as-is
                    quote! { #item }
                }
            } else {
                // Keep non-method items (e.g., constants) as-is
                quote! { #item }
            }
        })
        .collect();

    // Rebuild the `impl` block
    let modified_impl_block = quote! {
        impl #self_ty {
            #(#modified_impl_items)*
        }
    };

    // Combine with the rest of the generated code
    let expanded = quote! {
        // Wrapper enum
        #wrapper_enum

        // Reconstructed impl block
        #modified_impl_block

        // `to_machine` method
        impl #self_ty {
            #to_machine_signature {
                #to_machine_checks
            }
        }

        impl #self_ty {
            #(#try_methods)*
        }
    };

    expanded.into()
}

fn parse_validators_attr(attr: TokenStream) -> syn::Result<(syn::Ident, syn::Ident)> {
    let parsed = syn::parse::<ValidatorsAttr>(attr)?;
    Ok((parsed.state, parsed.machine))
}

fn get_variants_of_state(state_ident: &syn::Ident) -> syn::Result<Vec<VariantInfo>> {
    // Convert the `syn::Ident` to a string: e.g. "TaskState"
    let enum_name = state_ident.to_string();

    // Attempt to retrieve the variants from STATE_VARIANTS
    match get_state_variants(&enum_name) {
        Some(variants) => Ok(variants),
        None => Err(syn::Error::new_spanned(
            state_ident,
            format!(
                "No variants found for enum `{}`. Did you mark it with #[state]?",
                enum_name
            ),
        )),
    }
}

fn build_to_machine_fn(
    enum_variants: &[VariantInfo],
    is_fns: &[&syn::ImplItemFn],
    machine_ident: &syn::Ident,
    wrapper_enum_ident: &syn::Ident,
) -> (proc_macro2::TokenStream, bool) {
    let machine_name_str = machine_ident.to_string();
    let field_names_opt = get_machine_fields(&machine_name_str);
    let field_idents = if let Some(field_names) = field_names_opt {
        field_names
            .into_iter()
            .map(|s| format_ident!("{}", s.0))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let mut checks = vec![];
    let mut has_async = false;

    for variant_info in enum_variants {
        let variant = &variant_info.name;
        let variant_snake = to_snake_case(variant);
        let is_method_ident = format_ident!("is_{}", variant_snake);
        let variant_ident = format_ident!("{}", variant);

        let user_fn = is_fns.iter().find(|f| f.sig.ident == is_method_ident);

        if let Some(f) = user_fn {
            let is_async = f.sig.asyncness.is_some();
            if is_async {
                has_async = true;
            }
            let await_token = if is_async {
                quote! { .await }
            } else {
                quote! {}
            };

            if let Some((ok_ty_opt, _err_ty_opt)) = extract_result_ok_err_types(&f.sig.output) {
                let expects_data = variant_info.data_type.is_some();

                match (expects_data, ok_ty_opt) {
                    (true, Some(_ty)) => {
                        // Data-bearing async or sync validator
                        checks.push(quote! {
                            if let Ok(data) = self.#is_method_ident(#(&#field_idents),*)#await_token {
                                let machine = #machine_ident::<#variant_ident>::new(#(#field_idents.clone()),*).transition_with(data);
                                return Ok(#wrapper_enum_ident::#variant_ident(machine));
                            }
                        });
                    }
                    (false, Some(Type::Tuple(t))) if t.elems.is_empty() => {
                        // No-data async or sync validator
                        checks.push(quote! {
                            if let Ok(()) = self.#is_method_ident(#(&#field_idents),*)#await_token {
                                let machine = #machine_ident::<#variant_ident>::new(#(#field_idents.clone()),*);
                                return Ok(#wrapper_enum_ident::#variant_ident(machine));
                            }
                        });
                    }
                    _ => {
                        checks.push(
                            syn::Error::new_spanned(
                                &f.sig.output,
                                "Invalid return type for validator",
                            )
                            .to_compile_error(),
                        );
                    }
                }
            }
        } else {
            checks.push(
                syn::Error::new(
                    proc_macro2::Span::call_site(),
                    format!("Missing validator method for variant '{}'", variant),
                )
                .to_compile_error(),
            );
        }
    }

    let generated_checks = quote! {
        #(#checks)*

        Err(statum::Error::InvalidState)
    };

    (generated_checks, has_async)
}

/// Helper to parse something like `-> Result<T, E>`
/// Returns `Some((Some(T), Some(E)))` or `Some((Some(T), None))` etc.
/// If it doesn't match a `Result<_, _>` signature, returns `None`.
fn extract_result_ok_err_types(ret: &ReturnType) -> Option<(Option<Type>, Option<Type>)> {
    if let ReturnType::Type(_, ty) = ret {
        if let Type::Path(type_path) = &**ty {
            let segments = &type_path.path.segments;
            if let Some(seg) = segments.last() {
                if seg.ident == "Result" {
                    // We expect <T, E>
                    if let PathArguments::AngleBracketed(args) = &seg.arguments {
                        let args = &args.args;
                        if args.len() == 2 {
                            let mut iter = args.iter();
                            let first = iter.next().unwrap();
                            let second = iter.next().unwrap();

                            // Convert to Type
                            let ok_ty = match first {
                                syn::GenericArgument::Type(t) => Some(t.clone()),
                                _ => None,
                            };
                            let err_ty = match second {
                                syn::GenericArgument::Type(t) => Some(t.clone()),
                                _ => None,
                            };
                            return Some((ok_ty, err_ty));
                        }
                    }
                }
            }
        }
    }
    None
}
