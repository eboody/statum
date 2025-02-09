#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2021::*;
#[macro_use]
extern crate std;
use proc_macro::TokenStream;
use quote::format_ident;
use quote::quote;
use quote::ToTokens;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::Ident;
use syn::{
    parse::Parser, parse_macro_input, punctuated::Punctuated, Data, DeriveInput, Fields,
    ItemImpl, Path, PathArguments, ReturnType, Token, Type,
};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;
struct VariantInfo {
    name: String,
    data_type: Option<String>,
}
#[automatically_derived]
impl ::core::clone::Clone for VariantInfo {
    #[inline]
    fn clone(&self) -> VariantInfo {
        VariantInfo {
            name: ::core::clone::Clone::clone(&self.name),
            data_type: ::core::clone::Clone::clone(&self.data_type),
        }
    }
}
#[automatically_derived]
impl ::core::fmt::Debug for VariantInfo {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field2_finish(
            f,
            "VariantInfo",
            "name",
            &self.name,
            "data_type",
            &&self.data_type,
        )
    }
}
static STATE_VARIANTS: OnceLock<Mutex<HashMap<String, Vec<VariantInfo>>>> = OnceLock::new();
static STATE_DERIVES: OnceLock<Mutex<String>> = OnceLock::new();
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
fn get_fields_map() -> &'static Mutex<HashMap<String, Vec<(String, String)>>> {
    MACHINE_FIELDS.get_or_init(|| Mutex::new(HashMap::new()))
}
pub(crate) fn register_machine_fields(enum_name: String, fields: Vec<(String, String)>) {
    let map = get_fields_map();
    map.lock().unwrap().insert(enum_name, fields);
}
pub(crate) fn get_machine_fields(enum_name: &str) -> Option<Vec<(String, String)>> {
    let map = get_fields_map();
    map.lock().unwrap().get(enum_name).cloned()
}
struct ModelAttr {
    machine: syn::Path,
    state: syn::Path,
}
#[automatically_derived]
impl ::core::clone::Clone for ModelAttr {
    #[inline]
    fn clone(&self) -> ModelAttr {
        ModelAttr {
            machine: ::core::clone::Clone::clone(&self.machine),
            state: ::core::clone::Clone::clone(&self.state),
        }
    }
}
struct ValidatorsAttr {
    state: Ident,
    machine: Ident,
}
#[automatically_derived]
impl ::core::clone::Clone for ValidatorsAttr {
    #[inline]
    fn clone(&self) -> ValidatorsAttr {
        ValidatorsAttr {
            state: ::core::clone::Clone::clone(&self.state),
            machine: ::core::clone::Clone::clone(&self.machine),
        }
    }
}
#[automatically_derived]
impl ::core::fmt::Debug for ValidatorsAttr {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field2_finish(
            f,
            "ValidatorsAttr",
            "state",
            &self.state,
            "machine",
            &&self.machine,
        )
    }
}
impl Parse for ValidatorsAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Ident>()?;
        input.parse::<::syn::token::Eq>()?;
        let state = input.parse()?;
        input.parse::<::syn::token::Comma>()?;
        input.parse::<Ident>()?;
        input.parse::<::syn::token::Eq>()?;
        let machine = input.parse()?;
        Ok(ValidatorsAttr { state, machine })
    }
}
impl syn::parse::Parse for ModelAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut machine = None;
        let mut state = None;
        let name1: syn::Ident = input.parse()?;
        input.parse::<::syn::token::Eq>()?;
        let value1: syn::Path = input.parse()?;
        match name1.to_string().as_str() {
            "machine" => machine = Some(value1),
            "state" => state = Some(value1),
            _ => {
                return Err(
                    syn::Error::new(name1.span(), "Expected 'machine' or 'state'"),
                );
            }
        }
        input.parse::<::syn::token::Comma>()?;
        let name2: syn::Ident = input.parse()?;
        input.parse::<::syn::token::Eq>()?;
        let value2: syn::Path = input.parse()?;
        match name2.to_string().as_str() {
            "machine" => {
                if machine.is_some() {
                    return Err(
                        syn::Error::new(name2.span(), "Duplicate 'machine' parameter"),
                    );
                }
                machine = Some(value2);
            }
            "state" => {
                if state.is_some() {
                    return Err(
                        syn::Error::new(name2.span(), "Duplicate 'state' parameter"),
                    );
                }
                state = Some(value2);
            }
            _ => {
                return Err(
                    syn::Error::new(name2.span(), "Expected 'machine' or 'state'"),
                );
            }
        }
        match (machine, state) {
            (Some(machine), Some(state)) => Ok(ModelAttr { machine, state }),
            _ => {
                Err(
                    syn::Error::new(
                        name1.span(),
                        "Must specify both 'machine' and 'state'",
                    ),
                )
            }
        }
    }
}
fn get_field_info(input: &DeriveInput) -> (Vec<&syn::Ident>, Vec<&syn::Type>) {
    match &input.data {
        syn::Data::Struct(s) => {
            match &s.fields {
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
                _ => {
                    ::core::panicking::panic_fmt(
                        format_args!("Only named fields are supported"),
                    );
                }
            }
        }
        _ => {
            ::core::panicking::panic_fmt(format_args!("Only structs are supported"));
        }
    }
}
#[proc_macro_attribute]
pub fn state(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = match ::syn::parse::<DeriveInput>(item) {
        ::syn::__private::Ok(data) => data,
        ::syn::__private::Err(err) => {
            return ::syn::__private::TokenStream::from(err.to_compile_error());
        }
    };
    let vis = &input.vis;
    let name = &input.ident;
    let variants: Vec<VariantInfo> = match &input.data {
        Data::Enum(data_enum) => {
            data_enum
                .variants
                .iter()
                .map(|v| {
                    let name = v.ident.to_string();
                    let data_type = match &v.fields {
                        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
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
                        _ => None,
                    };
                    VariantInfo { name, data_type }
                })
                .collect()
        }
        _ => {
            ::core::panicking::panic_fmt(
                format_args!("#[state] can only be used on enums"),
            );
        }
    };
    register_state_variants(name.to_string(), variants);
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
    let mut trait_bounds = ::alloc::vec::Vec::new();
    if wants_debug {
        trait_bounds
            .push({
                let mut _s = ::quote::__private::TokenStream::new();
                ::quote::__private::push_ident(&mut _s, "std");
                ::quote::__private::push_colon2(&mut _s);
                ::quote::__private::push_ident(&mut _s, "fmt");
                ::quote::__private::push_colon2(&mut _s);
                ::quote::__private::push_ident(&mut _s, "Debug");
                _s
            });
    }
    let trait_bounds = if trait_bounds.is_empty() {
        ::quote::__private::TokenStream::new()
    } else {
        {
            let mut _s = ::quote::__private::TokenStream::new();
            ::quote::__private::push_colon(&mut _s);
            {
                use ::quote::__private::ext::*;
                let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                #[allow(unused_mut)]
                let (mut trait_bounds, i) = trait_bounds.quote_into_iter();
                let has_iter = has_iter | i;
                let _: ::quote::__private::HasIterator = has_iter;
                while true {
                    let trait_bounds = match trait_bounds.next() {
                        Some(_x) => ::quote::__private::RepInterp(_x),
                        None => break,
                    };
                    ::quote::ToTokens::to_tokens(&trait_bounds, &mut _s);
                    ::quote::__private::push_add(&mut _s);
                }
            }
            _s
        }
    };
    let replicate_derives = if user_derives.is_empty() {
        ::quote::__private::TokenStream::new()
    } else {
        {
            let mut _s = ::quote::__private::TokenStream::new();
            ::quote::__private::push_pound(&mut _s);
            ::quote::__private::push_group(
                &mut _s,
                ::quote::__private::Delimiter::Bracket,
                {
                    let mut _s = ::quote::__private::TokenStream::new();
                    ::quote::__private::push_ident(&mut _s, "derive");
                    ::quote::__private::push_group(
                        &mut _s,
                        ::quote::__private::Delimiter::Parenthesis,
                        {
                            let mut _s = ::quote::__private::TokenStream::new();
                            {
                                use ::quote::__private::ext::*;
                                let mut _i = 0usize;
                                let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                                #[allow(unused_mut)]
                                let (mut user_derives, i) = user_derives.quote_into_iter();
                                let has_iter = has_iter | i;
                                let _: ::quote::__private::HasIterator = has_iter;
                                while true {
                                    let user_derives = match user_derives.next() {
                                        Some(_x) => ::quote::__private::RepInterp(_x),
                                        None => break,
                                    };
                                    if _i > 0 {
                                        ::quote::__private::push_comma(&mut _s);
                                    }
                                    _i += 1;
                                    ::quote::ToTokens::to_tokens(&user_derives, &mut _s);
                                }
                            }
                            _s
                        },
                    );
                    _s
                },
            );
            _s
        }
    };
    let replicate_string = replicate_derives.to_string();
    if !replicate_string.is_empty() {
        let state_derives_ref = STATE_DERIVES.get_or_init(|| Mutex::new(String::new()));
        state_derives_ref.lock().unwrap().push_str(&replicate_string);
        {
            ::std::io::_print(
                format_args!("replicate_derives: {0}\n", replicate_derives),
            );
        };
    }
    let states = match &input.data {
        Data::Enum(data_enum) => {
            data_enum
                .variants
                .iter()
                .map(|variant| {
                    let variant_ident = &variant.ident;
                    let variant_fields = &variant.fields;
                    match variant_fields {
                        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                            let field_type = &fields.unnamed.first().unwrap().ty;
                            {
                                let mut _s = ::quote::__private::TokenStream::new();
                                ::quote::ToTokens::to_tokens(&replicate_derives, &mut _s);
                                ::quote::ToTokens::to_tokens(&vis, &mut _s);
                                ::quote::__private::push_ident(&mut _s, "struct");
                                ::quote::ToTokens::to_tokens(&variant_ident, &mut _s);
                                ::quote::__private::push_group(
                                    &mut _s,
                                    ::quote::__private::Delimiter::Parenthesis,
                                    {
                                        let mut _s = ::quote::__private::TokenStream::new();
                                        ::quote::ToTokens::to_tokens(&field_type, &mut _s);
                                        _s
                                    },
                                );
                                ::quote::__private::push_semi(&mut _s);
                                ::quote::__private::push_ident(&mut _s, "impl");
                                ::quote::ToTokens::to_tokens(&name, &mut _s);
                                ::quote::__private::push_ident(&mut _s, "for");
                                ::quote::ToTokens::to_tokens(&variant_ident, &mut _s);
                                ::quote::__private::push_group(
                                    &mut _s,
                                    ::quote::__private::Delimiter::Brace,
                                    {
                                        let mut _s = ::quote::__private::TokenStream::new();
                                        ::quote::__private::push_ident(&mut _s, "type");
                                        ::quote::__private::push_ident(&mut _s, "Data");
                                        ::quote::__private::push_eq(&mut _s);
                                        ::quote::ToTokens::to_tokens(&field_type, &mut _s);
                                        ::quote::__private::push_semi(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "const");
                                        ::quote::__private::push_ident(&mut _s, "HAS_DATA");
                                        ::quote::__private::push_colon(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "bool");
                                        ::quote::__private::push_eq(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "true");
                                        ::quote::__private::push_semi(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "fn");
                                        ::quote::__private::push_ident(&mut _s, "get_data");
                                        ::quote::__private::push_group(
                                            &mut _s,
                                            ::quote::__private::Delimiter::Parenthesis,
                                            {
                                                let mut _s = ::quote::__private::TokenStream::new();
                                                ::quote::__private::push_and(&mut _s);
                                                ::quote::__private::push_ident(&mut _s, "self");
                                                _s
                                            },
                                        );
                                        ::quote::__private::push_rarrow(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "Option");
                                        ::quote::__private::push_lt(&mut _s);
                                        ::quote::__private::push_and(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "Self");
                                        ::quote::__private::push_colon2(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "Data");
                                        ::quote::__private::push_gt(&mut _s);
                                        ::quote::__private::push_group(
                                            &mut _s,
                                            ::quote::__private::Delimiter::Brace,
                                            {
                                                let mut _s = ::quote::__private::TokenStream::new();
                                                ::quote::__private::push_ident(&mut _s, "Some");
                                                ::quote::__private::push_group(
                                                    &mut _s,
                                                    ::quote::__private::Delimiter::Parenthesis,
                                                    {
                                                        let mut _s = ::quote::__private::TokenStream::new();
                                                        ::quote::__private::push_and(&mut _s);
                                                        ::quote::__private::push_ident(&mut _s, "self");
                                                        ::quote::__private::push_dot(&mut _s);
                                                        ::quote::__private::parse(&mut _s, "0");
                                                        _s
                                                    },
                                                );
                                                _s
                                            },
                                        );
                                        ::quote::__private::push_ident(&mut _s, "fn");
                                        ::quote::__private::push_ident(&mut _s, "get_data_mut");
                                        ::quote::__private::push_group(
                                            &mut _s,
                                            ::quote::__private::Delimiter::Parenthesis,
                                            {
                                                let mut _s = ::quote::__private::TokenStream::new();
                                                ::quote::__private::push_and(&mut _s);
                                                ::quote::__private::push_ident(&mut _s, "mut");
                                                ::quote::__private::push_ident(&mut _s, "self");
                                                _s
                                            },
                                        );
                                        ::quote::__private::push_rarrow(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "Option");
                                        ::quote::__private::push_lt(&mut _s);
                                        ::quote::__private::push_and(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "mut");
                                        ::quote::__private::push_ident(&mut _s, "Self");
                                        ::quote::__private::push_colon2(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "Data");
                                        ::quote::__private::push_gt(&mut _s);
                                        ::quote::__private::push_group(
                                            &mut _s,
                                            ::quote::__private::Delimiter::Brace,
                                            {
                                                let mut _s = ::quote::__private::TokenStream::new();
                                                ::quote::__private::push_ident(&mut _s, "Some");
                                                ::quote::__private::push_group(
                                                    &mut _s,
                                                    ::quote::__private::Delimiter::Parenthesis,
                                                    {
                                                        let mut _s = ::quote::__private::TokenStream::new();
                                                        ::quote::__private::push_and(&mut _s);
                                                        ::quote::__private::push_ident(&mut _s, "mut");
                                                        ::quote::__private::push_ident(&mut _s, "self");
                                                        ::quote::__private::push_dot(&mut _s);
                                                        ::quote::__private::parse(&mut _s, "0");
                                                        _s
                                                    },
                                                );
                                                _s
                                            },
                                        );
                                        _s
                                    },
                                );
                                _s
                            }
                        }
                        Fields::Unit => {
                            let mut _s = ::quote::__private::TokenStream::new();
                            ::quote::ToTokens::to_tokens(&replicate_derives, &mut _s);
                            ::quote::ToTokens::to_tokens(&vis, &mut _s);
                            ::quote::__private::push_ident(&mut _s, "struct");
                            ::quote::ToTokens::to_tokens(&variant_ident, &mut _s);
                            ::quote::__private::push_semi(&mut _s);
                            ::quote::__private::push_ident(&mut _s, "impl");
                            ::quote::ToTokens::to_tokens(&name, &mut _s);
                            ::quote::__private::push_ident(&mut _s, "for");
                            ::quote::ToTokens::to_tokens(&variant_ident, &mut _s);
                            ::quote::__private::push_group(
                                &mut _s,
                                ::quote::__private::Delimiter::Brace,
                                {
                                    let mut _s = ::quote::__private::TokenStream::new();
                                    ::quote::__private::push_ident(&mut _s, "type");
                                    ::quote::__private::push_ident(&mut _s, "Data");
                                    ::quote::__private::push_eq(&mut _s);
                                    ::quote::__private::push_group(
                                        &mut _s,
                                        ::quote::__private::Delimiter::Parenthesis,
                                        ::quote::__private::TokenStream::new(),
                                    );
                                    ::quote::__private::push_semi(&mut _s);
                                    ::quote::__private::push_ident(&mut _s, "const");
                                    ::quote::__private::push_ident(&mut _s, "HAS_DATA");
                                    ::quote::__private::push_colon(&mut _s);
                                    ::quote::__private::push_ident(&mut _s, "bool");
                                    ::quote::__private::push_eq(&mut _s);
                                    ::quote::__private::push_ident(&mut _s, "false");
                                    ::quote::__private::push_semi(&mut _s);
                                    ::quote::__private::push_ident(&mut _s, "fn");
                                    ::quote::__private::push_ident(&mut _s, "get_data");
                                    ::quote::__private::push_group(
                                        &mut _s,
                                        ::quote::__private::Delimiter::Parenthesis,
                                        {
                                            let mut _s = ::quote::__private::TokenStream::new();
                                            ::quote::__private::push_and(&mut _s);
                                            ::quote::__private::push_ident(&mut _s, "self");
                                            _s
                                        },
                                    );
                                    ::quote::__private::push_rarrow(&mut _s);
                                    ::quote::__private::push_ident(&mut _s, "Option");
                                    ::quote::__private::push_lt(&mut _s);
                                    ::quote::__private::push_and(&mut _s);
                                    ::quote::__private::push_ident(&mut _s, "Self");
                                    ::quote::__private::push_colon2(&mut _s);
                                    ::quote::__private::push_ident(&mut _s, "Data");
                                    ::quote::__private::push_gt(&mut _s);
                                    ::quote::__private::push_group(
                                        &mut _s,
                                        ::quote::__private::Delimiter::Brace,
                                        {
                                            let mut _s = ::quote::__private::TokenStream::new();
                                            ::quote::__private::push_ident(&mut _s, "None");
                                            _s
                                        },
                                    );
                                    ::quote::__private::push_ident(&mut _s, "fn");
                                    ::quote::__private::push_ident(&mut _s, "get_data_mut");
                                    ::quote::__private::push_group(
                                        &mut _s,
                                        ::quote::__private::Delimiter::Parenthesis,
                                        {
                                            let mut _s = ::quote::__private::TokenStream::new();
                                            ::quote::__private::push_and(&mut _s);
                                            ::quote::__private::push_ident(&mut _s, "mut");
                                            ::quote::__private::push_ident(&mut _s, "self");
                                            _s
                                        },
                                    );
                                    ::quote::__private::push_rarrow(&mut _s);
                                    ::quote::__private::push_ident(&mut _s, "Option");
                                    ::quote::__private::push_lt(&mut _s);
                                    ::quote::__private::push_and(&mut _s);
                                    ::quote::__private::push_ident(&mut _s, "mut");
                                    ::quote::__private::push_ident(&mut _s, "Self");
                                    ::quote::__private::push_colon2(&mut _s);
                                    ::quote::__private::push_ident(&mut _s, "Data");
                                    ::quote::__private::push_gt(&mut _s);
                                    ::quote::__private::push_group(
                                        &mut _s,
                                        ::quote::__private::Delimiter::Brace,
                                        {
                                            let mut _s = ::quote::__private::TokenStream::new();
                                            ::quote::__private::push_ident(&mut _s, "None");
                                            _s
                                        },
                                    );
                                    _s
                                },
                            );
                            _s
                        }
                        _ => {
                            ::core::panicking::panic_fmt(
                                format_args!(
                                    "Variants must be unit or single-field tuple variants",
                                ),
                            );
                        }
                    }
                })
        }
        _ => {
            return syn::Error::new_spanned(
                    &input.ident,
                    "#[state] can only be used on an enum",
                )
                .to_compile_error()
                .into();
        }
    };
    let expanded = {
        let mut _s = ::quote::__private::TokenStream::new();
        ::quote::ToTokens::to_tokens(&vis, &mut _s);
        ::quote::__private::push_ident(&mut _s, "trait");
        ::quote::ToTokens::to_tokens(&name, &mut _s);
        ::quote::__private::push_group(
            &mut _s,
            ::quote::__private::Delimiter::Brace,
            {
                let mut _s = ::quote::__private::TokenStream::new();
                ::quote::__private::push_ident(&mut _s, "type");
                ::quote::__private::push_ident(&mut _s, "Data");
                ::quote::ToTokens::to_tokens(&trait_bounds, &mut _s);
                ::quote::__private::push_semi(&mut _s);
                ::quote::__private::push_ident(&mut _s, "const");
                ::quote::__private::push_ident(&mut _s, "HAS_DATA");
                ::quote::__private::push_colon(&mut _s);
                ::quote::__private::push_ident(&mut _s, "bool");
                ::quote::__private::push_semi(&mut _s);
                ::quote::__private::push_ident(&mut _s, "fn");
                ::quote::__private::push_ident(&mut _s, "get_data");
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Parenthesis,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::__private::push_and(&mut _s);
                        ::quote::__private::push_ident(&mut _s, "self");
                        _s
                    },
                );
                ::quote::__private::push_rarrow(&mut _s);
                ::quote::__private::push_ident(&mut _s, "Option");
                ::quote::__private::push_lt(&mut _s);
                ::quote::__private::push_and(&mut _s);
                ::quote::__private::push_ident(&mut _s, "Self");
                ::quote::__private::push_colon2(&mut _s);
                ::quote::__private::push_ident(&mut _s, "Data");
                ::quote::__private::push_gt(&mut _s);
                ::quote::__private::push_semi(&mut _s);
                ::quote::__private::push_ident(&mut _s, "fn");
                ::quote::__private::push_ident(&mut _s, "get_data_mut");
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Parenthesis,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::__private::push_and(&mut _s);
                        ::quote::__private::push_ident(&mut _s, "mut");
                        ::quote::__private::push_ident(&mut _s, "self");
                        _s
                    },
                );
                ::quote::__private::push_rarrow(&mut _s);
                ::quote::__private::push_ident(&mut _s, "Option");
                ::quote::__private::push_lt(&mut _s);
                ::quote::__private::push_and(&mut _s);
                ::quote::__private::push_ident(&mut _s, "mut");
                ::quote::__private::push_ident(&mut _s, "Self");
                ::quote::__private::push_colon2(&mut _s);
                ::quote::__private::push_ident(&mut _s, "Data");
                ::quote::__private::push_gt(&mut _s);
                ::quote::__private::push_semi(&mut _s);
                _s
            },
        );
        {
            use ::quote::__private::ext::*;
            let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
            #[allow(unused_mut)]
            let (mut states, i) = states.quote_into_iter();
            let has_iter = has_iter | i;
            let _: ::quote::__private::HasIterator = has_iter;
            while true {
                let states = match states.next() {
                    Some(_x) => ::quote::__private::RepInterp(_x),
                    None => break,
                };
                ::quote::ToTokens::to_tokens(&states, &mut _s);
            }
        }
        _s
    };
    TokenStream::from(expanded)
}
#[proc_macro_attribute]
pub fn machine(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = match ::syn::parse::<DeriveInput>(item) {
        ::syn::__private::Ok(data) => data,
        ::syn::__private::Err(err) => {
            return ::syn::__private::TokenStream::from(err.to_compile_error());
        }
    };
    let struct_name = &input.ident;
    let state_trait = extract_state_trait(&input);
    if let syn::Data::Struct(ref mut struct_data) = input.data {
        if let syn::Fields::Named(ref mut fields) = struct_data.fields {
            fields
                .named
                .push(
                    syn::Field::parse_named
                        .parse2({
                            let mut _s = ::quote::__private::TokenStream::new();
                            ::quote::__private::push_ident(&mut _s, "marker");
                            ::quote::__private::push_colon(&mut _s);
                            ::quote::__private::push_ident(&mut _s, "core");
                            ::quote::__private::push_colon2(&mut _s);
                            ::quote::__private::push_ident(&mut _s, "marker");
                            ::quote::__private::push_colon2(&mut _s);
                            ::quote::__private::push_ident(&mut _s, "PhantomData");
                            ::quote::__private::push_lt(&mut _s);
                            ::quote::__private::push_ident(&mut _s, "S");
                            ::quote::__private::push_gt(&mut _s);
                            _s
                        })
                        .unwrap(),
                );
            fields
                .named
                .push(
                    syn::Field::parse_named
                        .parse2({
                            let mut _s = ::quote::__private::TokenStream::new();
                            ::quote::__private::push_ident(&mut _s, "state_data");
                            ::quote::__private::push_colon(&mut _s);
                            ::quote::__private::push_ident(&mut _s, "Option");
                            ::quote::__private::push_lt(&mut _s);
                            ::quote::__private::push_ident(&mut _s, "S");
                            ::quote::__private::push_colon2(&mut _s);
                            ::quote::__private::push_ident(&mut _s, "Data");
                            ::quote::__private::push_gt(&mut _s);
                            _s
                        })
                        .unwrap(),
                );
        }
    }
    let (field_names, field_types) = get_field_info(&input);
    let fields_with_types: Vec<(String, String)> = field_names
        .iter()
        .zip(field_types.iter())
        .map(|(name, ty)| (name.to_string(), ty.to_token_stream().to_string()))
        .collect();
    register_machine_fields(struct_name.to_string(), fields_with_types);
    let transition_impl = {
        let mut _s = ::quote::__private::TokenStream::new();
        ::quote::__private::push_ident(&mut _s, "impl");
        ::quote::__private::push_lt(&mut _s);
        ::quote::__private::push_ident(&mut _s, "CurrentState");
        ::quote::__private::push_colon(&mut _s);
        ::quote::ToTokens::to_tokens(&state_trait, &mut _s);
        ::quote::__private::push_gt(&mut _s);
        ::quote::ToTokens::to_tokens(&struct_name, &mut _s);
        ::quote::__private::push_lt(&mut _s);
        ::quote::__private::push_ident(&mut _s, "CurrentState");
        ::quote::__private::push_gt(&mut _s);
        ::quote::__private::push_group(
            &mut _s,
            ::quote::__private::Delimiter::Brace,
            {
                let mut _s = ::quote::__private::TokenStream::new();
                ::quote::__private::push_ident(&mut _s, "pub");
                ::quote::__private::push_ident(&mut _s, "fn");
                ::quote::__private::push_ident(&mut _s, "transition");
                ::quote::__private::push_lt(&mut _s);
                ::quote::__private::push_ident(&mut _s, "NewState");
                ::quote::__private::push_colon(&mut _s);
                ::quote::ToTokens::to_tokens(&state_trait, &mut _s);
                ::quote::__private::push_gt(&mut _s);
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Parenthesis,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::__private::push_ident(&mut _s, "self");
                        _s
                    },
                );
                ::quote::__private::push_rarrow(&mut _s);
                ::quote::ToTokens::to_tokens(&struct_name, &mut _s);
                ::quote::__private::push_lt(&mut _s);
                ::quote::__private::push_ident(&mut _s, "NewState");
                ::quote::__private::push_gt(&mut _s);
                ::quote::__private::push_ident(&mut _s, "where");
                ::quote::__private::push_ident(&mut _s, "NewState");
                ::quote::__private::push_colon(&mut _s);
                ::quote::ToTokens::to_tokens(&state_trait, &mut _s);
                ::quote::__private::push_lt(&mut _s);
                ::quote::__private::push_ident(&mut _s, "Data");
                ::quote::__private::push_eq(&mut _s);
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Parenthesis,
                    ::quote::__private::TokenStream::new(),
                );
                ::quote::__private::push_gt(&mut _s);
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Brace,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::ToTokens::to_tokens(&struct_name, &mut _s);
                        ::quote::__private::push_group(
                            &mut _s,
                            ::quote::__private::Delimiter::Brace,
                            {
                                let mut _s = ::quote::__private::TokenStream::new();
                                {
                                    use ::quote::__private::ext::*;
                                    let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                                    #[allow(unused_mut)]
                                    let (mut field_names, i) = field_names.quote_into_iter();
                                    let has_iter = has_iter | i;
                                    #[allow(unused_mut)]
                                    let (mut field_names, i) = field_names.quote_into_iter();
                                    let has_iter = has_iter | i;
                                    let _: ::quote::__private::HasIterator = has_iter;
                                    while true {
                                        let field_names = match field_names.next() {
                                            Some(_x) => ::quote::__private::RepInterp(_x),
                                            None => break,
                                        };
                                        let field_names = match field_names.next() {
                                            Some(_x) => ::quote::__private::RepInterp(_x),
                                            None => break,
                                        };
                                        ::quote::ToTokens::to_tokens(&field_names, &mut _s);
                                        ::quote::__private::push_colon(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "self");
                                        ::quote::__private::push_dot(&mut _s);
                                        ::quote::ToTokens::to_tokens(&field_names, &mut _s);
                                        ::quote::__private::push_comma(&mut _s);
                                    }
                                }
                                ::quote::__private::push_ident(&mut _s, "marker");
                                ::quote::__private::push_colon(&mut _s);
                                ::quote::__private::push_ident(&mut _s, "core");
                                ::quote::__private::push_colon2(&mut _s);
                                ::quote::__private::push_ident(&mut _s, "marker");
                                ::quote::__private::push_colon2(&mut _s);
                                ::quote::__private::push_ident(&mut _s, "PhantomData");
                                ::quote::__private::push_comma(&mut _s);
                                ::quote::__private::push_ident(&mut _s, "state_data");
                                ::quote::__private::push_colon(&mut _s);
                                ::quote::__private::push_ident(&mut _s, "None");
                                ::quote::__private::push_comma(&mut _s);
                                _s
                            },
                        );
                        _s
                    },
                );
                ::quote::__private::push_ident(&mut _s, "pub");
                ::quote::__private::push_ident(&mut _s, "fn");
                ::quote::__private::push_ident(&mut _s, "transition_with");
                ::quote::__private::push_lt(&mut _s);
                ::quote::__private::push_ident(&mut _s, "NewState");
                ::quote::__private::push_colon(&mut _s);
                ::quote::ToTokens::to_tokens(&state_trait, &mut _s);
                ::quote::__private::push_gt(&mut _s);
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Parenthesis,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::__private::push_ident(&mut _s, "self");
                        ::quote::__private::push_comma(&mut _s);
                        ::quote::__private::push_ident(&mut _s, "data");
                        ::quote::__private::push_colon(&mut _s);
                        ::quote::__private::push_ident(&mut _s, "NewState");
                        ::quote::__private::push_colon2(&mut _s);
                        ::quote::__private::push_ident(&mut _s, "Data");
                        _s
                    },
                );
                ::quote::__private::push_rarrow(&mut _s);
                ::quote::ToTokens::to_tokens(&struct_name, &mut _s);
                ::quote::__private::push_lt(&mut _s);
                ::quote::__private::push_ident(&mut _s, "NewState");
                ::quote::__private::push_gt(&mut _s);
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Brace,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::ToTokens::to_tokens(&struct_name, &mut _s);
                        ::quote::__private::push_group(
                            &mut _s,
                            ::quote::__private::Delimiter::Brace,
                            {
                                let mut _s = ::quote::__private::TokenStream::new();
                                {
                                    use ::quote::__private::ext::*;
                                    let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                                    #[allow(unused_mut)]
                                    let (mut field_names, i) = field_names.quote_into_iter();
                                    let has_iter = has_iter | i;
                                    #[allow(unused_mut)]
                                    let (mut field_names, i) = field_names.quote_into_iter();
                                    let has_iter = has_iter | i;
                                    let _: ::quote::__private::HasIterator = has_iter;
                                    while true {
                                        let field_names = match field_names.next() {
                                            Some(_x) => ::quote::__private::RepInterp(_x),
                                            None => break,
                                        };
                                        let field_names = match field_names.next() {
                                            Some(_x) => ::quote::__private::RepInterp(_x),
                                            None => break,
                                        };
                                        ::quote::ToTokens::to_tokens(&field_names, &mut _s);
                                        ::quote::__private::push_colon(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "self");
                                        ::quote::__private::push_dot(&mut _s);
                                        ::quote::ToTokens::to_tokens(&field_names, &mut _s);
                                        ::quote::__private::push_comma(&mut _s);
                                    }
                                }
                                ::quote::__private::push_ident(&mut _s, "marker");
                                ::quote::__private::push_colon(&mut _s);
                                ::quote::__private::push_ident(&mut _s, "core");
                                ::quote::__private::push_colon2(&mut _s);
                                ::quote::__private::push_ident(&mut _s, "marker");
                                ::quote::__private::push_colon2(&mut _s);
                                ::quote::__private::push_ident(&mut _s, "PhantomData");
                                ::quote::__private::push_comma(&mut _s);
                                ::quote::__private::push_ident(&mut _s, "state_data");
                                ::quote::__private::push_colon(&mut _s);
                                ::quote::__private::push_ident(&mut _s, "Some");
                                ::quote::__private::push_group(
                                    &mut _s,
                                    ::quote::__private::Delimiter::Parenthesis,
                                    {
                                        let mut _s = ::quote::__private::TokenStream::new();
                                        ::quote::__private::push_ident(&mut _s, "data");
                                        _s
                                    },
                                );
                                ::quote::__private::push_comma(&mut _s);
                                _s
                            },
                        );
                        _s
                    },
                );
                ::quote::__private::push_ident(&mut _s, "pub");
                ::quote::__private::push_ident(&mut _s, "fn");
                ::quote::__private::push_ident(&mut _s, "get_state_data");
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Parenthesis,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::__private::push_and(&mut _s);
                        ::quote::__private::push_ident(&mut _s, "self");
                        _s
                    },
                );
                ::quote::__private::push_rarrow(&mut _s);
                ::quote::__private::push_ident(&mut _s, "Option");
                ::quote::__private::push_lt(&mut _s);
                ::quote::__private::push_and(&mut _s);
                ::quote::__private::push_ident(&mut _s, "CurrentState");
                ::quote::__private::push_colon2(&mut _s);
                ::quote::__private::push_ident(&mut _s, "Data");
                ::quote::__private::push_gt(&mut _s);
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Brace,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::__private::push_ident(&mut _s, "self");
                        ::quote::__private::push_dot(&mut _s);
                        ::quote::__private::push_ident(&mut _s, "state_data");
                        ::quote::__private::push_dot(&mut _s);
                        ::quote::__private::push_ident(&mut _s, "as_ref");
                        ::quote::__private::push_group(
                            &mut _s,
                            ::quote::__private::Delimiter::Parenthesis,
                            ::quote::__private::TokenStream::new(),
                        );
                        _s
                    },
                );
                ::quote::__private::push_ident(&mut _s, "pub");
                ::quote::__private::push_ident(&mut _s, "fn");
                ::quote::__private::push_ident(&mut _s, "get_state_data_mut");
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Parenthesis,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::__private::push_and(&mut _s);
                        ::quote::__private::push_ident(&mut _s, "mut");
                        ::quote::__private::push_ident(&mut _s, "self");
                        _s
                    },
                );
                ::quote::__private::push_rarrow(&mut _s);
                ::quote::__private::push_ident(&mut _s, "Option");
                ::quote::__private::push_lt(&mut _s);
                ::quote::__private::push_and(&mut _s);
                ::quote::__private::push_ident(&mut _s, "mut");
                ::quote::__private::push_ident(&mut _s, "CurrentState");
                ::quote::__private::push_colon2(&mut _s);
                ::quote::__private::push_ident(&mut _s, "Data");
                ::quote::__private::push_gt(&mut _s);
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Brace,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::__private::push_ident(&mut _s, "self");
                        ::quote::__private::push_dot(&mut _s);
                        ::quote::__private::push_ident(&mut _s, "state_data");
                        ::quote::__private::push_dot(&mut _s);
                        ::quote::__private::push_ident(&mut _s, "as_mut");
                        ::quote::__private::push_group(
                            &mut _s,
                            ::quote::__private::Delimiter::Parenthesis,
                            ::quote::__private::TokenStream::new(),
                        );
                        _s
                    },
                );
                _s
            },
        );
        _s
    };
    let constructor = {
        let mut _s = ::quote::__private::TokenStream::new();
        ::quote::__private::push_ident(&mut _s, "impl");
        ::quote::__private::push_lt(&mut _s);
        ::quote::__private::push_ident(&mut _s, "S");
        ::quote::__private::push_colon(&mut _s);
        ::quote::ToTokens::to_tokens(&state_trait, &mut _s);
        ::quote::__private::push_gt(&mut _s);
        ::quote::ToTokens::to_tokens(&struct_name, &mut _s);
        ::quote::__private::push_lt(&mut _s);
        ::quote::__private::push_ident(&mut _s, "S");
        ::quote::__private::push_gt(&mut _s);
        ::quote::__private::push_group(
            &mut _s,
            ::quote::__private::Delimiter::Brace,
            {
                let mut _s = ::quote::__private::TokenStream::new();
                ::quote::__private::push_ident(&mut _s, "pub");
                ::quote::__private::push_ident(&mut _s, "fn");
                ::quote::__private::push_ident(&mut _s, "new");
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Parenthesis,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        {
                            use ::quote::__private::ext::*;
                            let mut _i = 0usize;
                            let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                            #[allow(unused_mut)]
                            let (mut field_names, i) = field_names.quote_into_iter();
                            let has_iter = has_iter | i;
                            #[allow(unused_mut)]
                            let (mut field_types, i) = field_types.quote_into_iter();
                            let has_iter = has_iter | i;
                            let _: ::quote::__private::HasIterator = has_iter;
                            while true {
                                let field_names = match field_names.next() {
                                    Some(_x) => ::quote::__private::RepInterp(_x),
                                    None => break,
                                };
                                let field_types = match field_types.next() {
                                    Some(_x) => ::quote::__private::RepInterp(_x),
                                    None => break,
                                };
                                if _i > 0 {
                                    ::quote::__private::push_comma(&mut _s);
                                }
                                _i += 1;
                                ::quote::ToTokens::to_tokens(&field_names, &mut _s);
                                ::quote::__private::push_colon(&mut _s);
                                ::quote::ToTokens::to_tokens(&field_types, &mut _s);
                            }
                        }
                        _s
                    },
                );
                ::quote::__private::push_rarrow(&mut _s);
                ::quote::__private::push_ident(&mut _s, "Self");
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Brace,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::__private::push_ident(&mut _s, "Self");
                        ::quote::__private::push_group(
                            &mut _s,
                            ::quote::__private::Delimiter::Brace,
                            {
                                let mut _s = ::quote::__private::TokenStream::new();
                                {
                                    use ::quote::__private::ext::*;
                                    let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                                    #[allow(unused_mut)]
                                    let (mut field_names, i) = field_names.quote_into_iter();
                                    let has_iter = has_iter | i;
                                    let _: ::quote::__private::HasIterator = has_iter;
                                    while true {
                                        let field_names = match field_names.next() {
                                            Some(_x) => ::quote::__private::RepInterp(_x),
                                            None => break,
                                        };
                                        ::quote::ToTokens::to_tokens(&field_names, &mut _s);
                                        ::quote::__private::push_comma(&mut _s);
                                    }
                                }
                                ::quote::__private::push_ident(&mut _s, "marker");
                                ::quote::__private::push_colon(&mut _s);
                                ::quote::__private::push_ident(&mut _s, "core");
                                ::quote::__private::push_colon2(&mut _s);
                                ::quote::__private::push_ident(&mut _s, "marker");
                                ::quote::__private::push_colon2(&mut _s);
                                ::quote::__private::push_ident(&mut _s, "PhantomData");
                                ::quote::__private::push_comma(&mut _s);
                                ::quote::__private::push_ident(&mut _s, "state_data");
                                ::quote::__private::push_colon(&mut _s);
                                ::quote::__private::push_ident(&mut _s, "None");
                                ::quote::__private::push_comma(&mut _s);
                                _s
                            },
                        );
                        _s
                    },
                );
                _s
            },
        );
        _s
    };
    let expanded = {
        let mut _s = ::quote::__private::TokenStream::new();
        ::quote::ToTokens::to_tokens(&input, &mut _s);
        ::quote::ToTokens::to_tokens(&transition_impl, &mut _s);
        ::quote::ToTokens::to_tokens(&constructor, &mut _s);
        _s
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
    {
        ::core::panicking::panic_fmt(
            format_args!("Type parameter must have a trait bound"),
        );
    };
}
fn analyze_user_derives(
    attrs: &[syn::Attribute],
) -> (Vec<Path>, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool, bool) {
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
    for attr in attrs {
        if attr.path().is_ident("derive") {
            if let Ok(paths) = attr
                .parse_args_with(
                    Punctuated::<Path, ::syn::token::Comma>::parse_terminated,
                )
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
    let ModelAttr { machine, state } = match ::syn::parse::<ModelAttr>(attr) {
        ::syn::__private::Ok(data) => data,
        ::syn::__private::Err(err) => {
            return ::syn::__private::TokenStream::from(err.to_compile_error());
        }
    };
    let input = match ::syn::parse::<DeriveInput>(item) {
        ::syn::__private::Ok(data) => data,
        ::syn::__private::Err(err) => {
            return ::syn::__private::TokenStream::from(err.to_compile_error());
        }
    };
    let struct_name = &input.ident;
    let machine_input = syn::parse_str::<
        DeriveInput,
    >(&machine.to_token_stream().to_string())
        .expect("Could not parse machine type");
    let (field_names, field_types) = get_field_info(&machine_input);
    let state_name = state.get_ident().expect("Expected simple state name").to_string();
    let variants = get_state_variants(&state_name)
        .expect("State type not found - did you mark it with #[state]?");
    let try_methods = variants
        .iter()
        .map(|variant| {
            let variant_ident = match ::quote::__private::IdentFragmentAdapter(
                &variant.name,
            ) {
                arg => {
                    ::quote::__private::mk_ident(
                        &::alloc::__export::must_use({
                            let res = ::alloc::fmt::format(format_args!("{0}", arg));
                            res
                        }),
                        ::quote::__private::Option::None.or(arg.span()),
                    )
                }
            };
            let try_method_name = match ::quote::__private::IdentFragmentAdapter(
                &to_snake_case(&variant.name),
            ) {
                arg => {
                    ::quote::__private::mk_ident(
                        &::alloc::__export::must_use({
                            let res = ::alloc::fmt::format(
                                format_args!("try_to_{0}", arg),
                            );
                            res
                        }),
                        ::quote::__private::Option::None.or(arg.span()),
                    )
                }
            };
            let is_method_name = match ::quote::__private::IdentFragmentAdapter(
                &to_snake_case(&variant.name),
            ) {
                arg => {
                    ::quote::__private::mk_ident(
                        &::alloc::__export::must_use({
                            let res = ::alloc::fmt::format(format_args!("is_{0}", arg));
                            res
                        }),
                        ::quote::__private::Option::None.or(arg.span()),
                    )
                }
            };
            {
                let mut _s = ::quote::__private::TokenStream::new();
                ::quote::__private::push_ident(&mut _s, "pub");
                ::quote::__private::push_ident(&mut _s, "fn");
                ::quote::ToTokens::to_tokens(&try_method_name, &mut _s);
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Parenthesis,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::__private::push_and(&mut _s);
                        ::quote::__private::push_ident(&mut _s, "self");
                        ::quote::__private::push_comma(&mut _s);
                        {
                            use ::quote::__private::ext::*;
                            let mut _i = 0usize;
                            let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                            #[allow(unused_mut)]
                            let (mut field_names, i) = field_names.quote_into_iter();
                            let has_iter = has_iter | i;
                            #[allow(unused_mut)]
                            let (mut field_types, i) = field_types.quote_into_iter();
                            let has_iter = has_iter | i;
                            let _: ::quote::__private::HasIterator = has_iter;
                            while true {
                                let field_names = match field_names.next() {
                                    Some(_x) => ::quote::__private::RepInterp(_x),
                                    None => break,
                                };
                                let field_types = match field_types.next() {
                                    Some(_x) => ::quote::__private::RepInterp(_x),
                                    None => break,
                                };
                                if _i > 0 {
                                    ::quote::__private::push_comma(&mut _s);
                                }
                                _i += 1;
                                ::quote::ToTokens::to_tokens(&field_names, &mut _s);
                                ::quote::__private::push_colon(&mut _s);
                                ::quote::ToTokens::to_tokens(&field_types, &mut _s);
                            }
                        }
                        _s
                    },
                );
                ::quote::__private::push_rarrow(&mut _s);
                ::quote::__private::push_ident(&mut _s, "Result");
                ::quote::__private::push_lt(&mut _s);
                ::quote::ToTokens::to_tokens(&machine, &mut _s);
                ::quote::__private::push_lt(&mut _s);
                ::quote::ToTokens::to_tokens(&variant_ident, &mut _s);
                ::quote::__private::push_gt(&mut _s);
                ::quote::__private::push_comma(&mut _s);
                ::quote::__private::push_ident(&mut _s, "statum");
                ::quote::__private::push_colon2(&mut _s);
                ::quote::__private::push_ident(&mut _s, "Error");
                ::quote::__private::push_gt(&mut _s);
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Brace,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::__private::push_ident(&mut _s, "if");
                        ::quote::__private::push_ident(&mut _s, "self");
                        ::quote::__private::push_dot(&mut _s);
                        ::quote::ToTokens::to_tokens(&is_method_name, &mut _s);
                        ::quote::__private::push_group(
                            &mut _s,
                            ::quote::__private::Delimiter::Parenthesis,
                            ::quote::__private::TokenStream::new(),
                        );
                        ::quote::__private::push_group(
                            &mut _s,
                            ::quote::__private::Delimiter::Brace,
                            {
                                let mut _s = ::quote::__private::TokenStream::new();
                                ::quote::__private::push_ident(&mut _s, "Ok");
                                ::quote::__private::push_group(
                                    &mut _s,
                                    ::quote::__private::Delimiter::Parenthesis,
                                    {
                                        let mut _s = ::quote::__private::TokenStream::new();
                                        ::quote::ToTokens::to_tokens(&machine, &mut _s);
                                        ::quote::__private::push_colon2(&mut _s);
                                        ::quote::__private::push_lt(&mut _s);
                                        ::quote::ToTokens::to_tokens(&variant_ident, &mut _s);
                                        ::quote::__private::push_gt(&mut _s);
                                        ::quote::__private::push_colon2(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "new");
                                        ::quote::__private::push_group(
                                            &mut _s,
                                            ::quote::__private::Delimiter::Parenthesis,
                                            {
                                                let mut _s = ::quote::__private::TokenStream::new();
                                                {
                                                    use ::quote::__private::ext::*;
                                                    let mut _i = 0usize;
                                                    let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                                                    #[allow(unused_mut)]
                                                    let (mut field_names, i) = field_names.quote_into_iter();
                                                    let has_iter = has_iter | i;
                                                    let _: ::quote::__private::HasIterator = has_iter;
                                                    while true {
                                                        let field_names = match field_names.next() {
                                                            Some(_x) => ::quote::__private::RepInterp(_x),
                                                            None => break,
                                                        };
                                                        if _i > 0 {
                                                            ::quote::__private::push_comma(&mut _s);
                                                        }
                                                        _i += 1;
                                                        ::quote::ToTokens::to_tokens(&field_names, &mut _s);
                                                    }
                                                }
                                                _s
                                            },
                                        );
                                        _s
                                    },
                                );
                                _s
                            },
                        );
                        ::quote::__private::push_ident(&mut _s, "else");
                        ::quote::__private::push_group(
                            &mut _s,
                            ::quote::__private::Delimiter::Brace,
                            {
                                let mut _s = ::quote::__private::TokenStream::new();
                                ::quote::__private::push_ident(&mut _s, "Err");
                                ::quote::__private::push_group(
                                    &mut _s,
                                    ::quote::__private::Delimiter::Parenthesis,
                                    {
                                        let mut _s = ::quote::__private::TokenStream::new();
                                        ::quote::__private::push_ident(&mut _s, "statum");
                                        ::quote::__private::push_colon2(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "Error");
                                        ::quote::__private::push_colon2(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "InvalidState");
                                        _s
                                    },
                                );
                                _s
                            },
                        );
                        _s
                    },
                );
                _s
            }
        });
    let expanded = {
        let mut _s = ::quote::__private::TokenStream::new();
        ::quote::ToTokens::to_tokens(&input, &mut _s);
        ::quote::__private::push_ident(&mut _s, "impl");
        ::quote::ToTokens::to_tokens(&struct_name, &mut _s);
        ::quote::__private::push_group(
            &mut _s,
            ::quote::__private::Delimiter::Brace,
            {
                let mut _s = ::quote::__private::TokenStream::new();
                {
                    use ::quote::__private::ext::*;
                    let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                    #[allow(unused_mut)]
                    let (mut try_methods, i) = try_methods.quote_into_iter();
                    let has_iter = has_iter | i;
                    let _: ::quote::__private::HasIterator = has_iter;
                    while true {
                        let try_methods = match try_methods.next() {
                            Some(_x) => ::quote::__private::RepInterp(_x),
                            None => break,
                        };
                        ::quote::ToTokens::to_tokens(&try_methods, &mut _s);
                    }
                }
                _s
            },
        );
        _s
    };
    TokenStream::from(expanded)
}
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
    let (state_ident, machine_ident) = match parse_validators_attr(attr) {
        Ok(pair) => pair,
        Err(e) => return e.to_compile_error().into(),
    };
    let impl_block = match ::syn::parse::<ItemImpl>(item) {
        ::syn::__private::Ok(data) => data,
        ::syn::__private::Err(err) => {
            return ::syn::__private::TokenStream::from(err.to_compile_error());
        }
    };
    let self_ty = &impl_block.self_ty;
    let enum_variants = match get_variants_of_state(&state_ident) {
        Ok(vars) => vars,
        Err(e) => return e.to_compile_error().into(),
    };
    let wrapper_enum_ident = match ::quote::__private::IdentFragmentAdapter(
        &machine_ident,
    ) {
        arg => {
            ::quote::__private::mk_ident(
                &::alloc::__export::must_use({
                    let res = ::alloc::fmt::format(format_args!("{0}State", arg));
                    res
                }),
                ::quote::__private::Option::None.or(arg.span()),
            )
        }
    };
    let wrapper_variants = enum_variants
        .iter()
        .map(|variant| {
            let v_id = match ::quote::__private::IdentFragmentAdapter(&variant.name) {
                arg => {
                    ::quote::__private::mk_ident(
                        &::alloc::__export::must_use({
                            let res = ::alloc::fmt::format(format_args!("{0}", arg));
                            res
                        }),
                        ::quote::__private::Option::None.or(arg.span()),
                    )
                }
            };
            {
                let mut _s = ::quote::__private::TokenStream::new();
                ::quote::ToTokens::to_tokens(&v_id, &mut _s);
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Parenthesis,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::ToTokens::to_tokens(&machine_ident, &mut _s);
                        ::quote::__private::push_lt(&mut _s);
                        ::quote::ToTokens::to_tokens(&v_id, &mut _s);
                        ::quote::__private::push_gt(&mut _s);
                        _s
                    },
                );
                _s
            }
        });
    let is_methods = enum_variants
        .iter()
        .map(|variant| {
            let variant_ident = match ::quote::__private::IdentFragmentAdapter(
                &variant.name,
            ) {
                arg => {
                    ::quote::__private::mk_ident(
                        &::alloc::__export::must_use({
                            let res = ::alloc::fmt::format(format_args!("{0}", arg));
                            res
                        }),
                        ::quote::__private::Option::None.or(arg.span()),
                    )
                }
            };
            let method_name = match ::quote::__private::IdentFragmentAdapter(
                &to_snake_case(&variant.name),
            ) {
                arg => {
                    ::quote::__private::mk_ident(
                        &::alloc::__export::must_use({
                            let res = ::alloc::fmt::format(format_args!("is_{0}", arg));
                            res
                        }),
                        ::quote::__private::Option::None.or(arg.span()),
                    )
                }
            };
            {
                let mut _s = ::quote::__private::TokenStream::new();
                ::quote::__private::push_ident(&mut _s, "pub");
                ::quote::__private::push_ident(&mut _s, "fn");
                ::quote::ToTokens::to_tokens(&method_name, &mut _s);
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Parenthesis,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::__private::push_and(&mut _s);
                        ::quote::__private::push_ident(&mut _s, "self");
                        _s
                    },
                );
                ::quote::__private::push_rarrow(&mut _s);
                ::quote::__private::push_ident(&mut _s, "bool");
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Brace,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::__private::push_ident(&mut _s, "matches");
                        ::quote::__private::push_bang(&mut _s);
                        ::quote::__private::push_group(
                            &mut _s,
                            ::quote::__private::Delimiter::Parenthesis,
                            {
                                let mut _s = ::quote::__private::TokenStream::new();
                                ::quote::__private::push_ident(&mut _s, "self");
                                ::quote::__private::push_comma(&mut _s);
                                ::quote::__private::push_ident(&mut _s, "Self");
                                ::quote::__private::push_colon2(&mut _s);
                                ::quote::ToTokens::to_tokens(&variant_ident, &mut _s);
                                ::quote::__private::push_group(
                                    &mut _s,
                                    ::quote::__private::Delimiter::Parenthesis,
                                    {
                                        let mut _s = ::quote::__private::TokenStream::new();
                                        ::quote::__private::push_underscore(&mut _s);
                                        _s
                                    },
                                );
                                _s
                            },
                        );
                        _s
                    },
                );
                _s
            }
        });
    let wrapper_variants_match_arms = enum_variants
        .iter()
        .map(|variant| {
            let variant_ident = match ::quote::__private::IdentFragmentAdapter(
                &variant.name,
            ) {
                arg => {
                    ::quote::__private::mk_ident(
                        &::alloc::__export::must_use({
                            let res = ::alloc::fmt::format(format_args!("{0}", arg));
                            res
                        }),
                        ::quote::__private::Option::None.or(arg.span()),
                    )
                }
            };
            {
                let mut _s = ::quote::__private::TokenStream::new();
                ::quote::__private::push_ident(&mut _s, "Self");
                ::quote::__private::push_colon2(&mut _s);
                ::quote::ToTokens::to_tokens(&variant_ident, &mut _s);
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Parenthesis,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::__private::push_ident(&mut _s, "machine");
                        _s
                    },
                );
                ::quote::__private::push_fat_arrow(&mut _s);
                ::quote::__private::push_ident(&mut _s, "Some");
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Parenthesis,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::__private::push_ident(&mut _s, "machine");
                        _s
                    },
                );
                _s
            }
        });
    let state_derives = if let Some(mutex) = STATE_DERIVES.get() {
        let state_derives = mutex.lock().unwrap();
        state_derives.clone().to_token_stream()
    } else {
        ::quote::__private::TokenStream::new()
    };
    {
        ::std::io::_print(
            format_args!(
                "state_derives: {0}\n",
                {
                    let mut _s = ::quote::__private::TokenStream::new();
                    ::quote::ToTokens::to_tokens(&state_derives, &mut _s);
                    _s
                },
            ),
        );
    };
    let wrapper_enum = {
        let mut _s = ::quote::__private::TokenStream::new();
        ::quote::ToTokens::to_tokens(&state_derives, &mut _s);
        ::quote::__private::push_ident(&mut _s, "pub");
        ::quote::__private::push_ident(&mut _s, "enum");
        ::quote::ToTokens::to_tokens(&wrapper_enum_ident, &mut _s);
        ::quote::__private::push_group(
            &mut _s,
            ::quote::__private::Delimiter::Brace,
            {
                let mut _s = ::quote::__private::TokenStream::new();
                {
                    use ::quote::__private::ext::*;
                    let mut _i = 0usize;
                    let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                    #[allow(unused_mut)]
                    let (mut wrapper_variants, i) = wrapper_variants.quote_into_iter();
                    let has_iter = has_iter | i;
                    let _: ::quote::__private::HasIterator = has_iter;
                    while true {
                        let wrapper_variants = match wrapper_variants.next() {
                            Some(_x) => ::quote::__private::RepInterp(_x),
                            None => break,
                        };
                        if _i > 0 {
                            ::quote::__private::push_comma(&mut _s);
                        }
                        _i += 1;
                        ::quote::ToTokens::to_tokens(&wrapper_variants, &mut _s);
                    }
                }
                _s
            },
        );
        ::quote::__private::push_ident(&mut _s, "impl");
        ::quote::ToTokens::to_tokens(&wrapper_enum_ident, &mut _s);
        ::quote::__private::push_group(
            &mut _s,
            ::quote::__private::Delimiter::Brace,
            {
                let mut _s = ::quote::__private::TokenStream::new();
                {
                    use ::quote::__private::ext::*;
                    let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                    #[allow(unused_mut)]
                    let (mut is_methods, i) = is_methods.quote_into_iter();
                    let has_iter = has_iter | i;
                    let _: ::quote::__private::HasIterator = has_iter;
                    while true {
                        let is_methods = match is_methods.next() {
                            Some(_x) => ::quote::__private::RepInterp(_x),
                            None => break,
                        };
                        ::quote::ToTokens::to_tokens(&is_methods, &mut _s);
                    }
                }
                ::quote::__private::push_ident(&mut _s, "pub");
                ::quote::__private::push_ident(&mut _s, "fn");
                ::quote::__private::push_ident(&mut _s, "as_ref");
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Parenthesis,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::__private::push_and(&mut _s);
                        ::quote::__private::push_ident(&mut _s, "self");
                        _s
                    },
                );
                ::quote::__private::push_rarrow(&mut _s);
                ::quote::__private::push_ident(&mut _s, "Option");
                ::quote::__private::push_lt(&mut _s);
                ::quote::__private::push_and(&mut _s);
                ::quote::__private::push_ident(&mut _s, "dyn");
                ::quote::__private::push_ident(&mut _s, "std");
                ::quote::__private::push_colon2(&mut _s);
                ::quote::__private::push_ident(&mut _s, "any");
                ::quote::__private::push_colon2(&mut _s);
                ::quote::__private::push_ident(&mut _s, "Any");
                ::quote::__private::push_gt(&mut _s);
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Brace,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::__private::push_ident(&mut _s, "match");
                        ::quote::__private::push_ident(&mut _s, "self");
                        ::quote::__private::push_group(
                            &mut _s,
                            ::quote::__private::Delimiter::Brace,
                            {
                                let mut _s = ::quote::__private::TokenStream::new();
                                {
                                    use ::quote::__private::ext::*;
                                    let mut _i = 0usize;
                                    let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                                    #[allow(unused_mut)]
                                    let (mut wrapper_variants_match_arms, i) = wrapper_variants_match_arms
                                        .quote_into_iter();
                                    let has_iter = has_iter | i;
                                    let _: ::quote::__private::HasIterator = has_iter;
                                    while true {
                                        let wrapper_variants_match_arms = match wrapper_variants_match_arms
                                            .next()
                                        {
                                            Some(_x) => ::quote::__private::RepInterp(_x),
                                            None => break,
                                        };
                                        if _i > 0 {
                                            ::quote::__private::push_comma(&mut _s);
                                        }
                                        _i += 1;
                                        ::quote::ToTokens::to_tokens(
                                            &wrapper_variants_match_arms,
                                            &mut _s,
                                        );
                                    }
                                }
                                _s
                            },
                        );
                        _s
                    },
                );
                _s
            },
        );
        _s
    };
    let machine_name_str = machine_ident.to_string();
    let field_names_opt = get_machine_fields(&machine_name_str);
    if field_names_opt.is_none() {
        return syn::Error::new_spanned(
                machine_ident,
                ::alloc::__export::must_use({
                    let res = ::alloc::fmt::format(
                        format_args!(
                            "Machine \'{0}\' does not have registered fields",
                            machine_name_str,
                        ),
                    );
                    res
                }),
            )
            .to_compile_error()
            .into();
    }
    let fields = field_names_opt.unwrap();
    let is_fns: Vec<&syn::ImplItemFn> = impl_block
        .items
        .iter()
        .filter_map(|item| {
            if let syn::ImplItem::Fn(func) = item { Some(func) } else { None }
        })
        .collect();
    let (to_machine_checks, has_async) = build_to_machine_fn(
        &enum_variants,
        &is_fns,
        &machine_ident,
        &wrapper_enum_ident,
    );
    let field_idents = fields
        .iter()
        .map(|(name, _type)| match ::quote::__private::IdentFragmentAdapter(&name) {
            arg => {
                ::quote::__private::mk_ident(
                    &::alloc::__export::must_use({
                        let res = ::alloc::fmt::format(format_args!("{0}", arg));
                        res
                    }),
                    ::quote::__private::Option::None.or(arg.span()),
                )
            }
        })
        .collect::<Vec<_>>();
    let field_types = fields
        .iter()
        .map(|(_name, ty)| syn::parse_str::<syn::Type>(ty).unwrap())
        .collect::<Vec<_>>();
    let to_machine_signature = if has_async {
        {
            let mut _s = ::quote::__private::TokenStream::new();
            ::quote::__private::push_ident(&mut _s, "pub");
            ::quote::__private::push_ident(&mut _s, "async");
            ::quote::__private::push_ident(&mut _s, "fn");
            ::quote::__private::push_ident(&mut _s, "to_machine");
            ::quote::__private::push_group(
                &mut _s,
                ::quote::__private::Delimiter::Parenthesis,
                {
                    let mut _s = ::quote::__private::TokenStream::new();
                    ::quote::__private::push_and(&mut _s);
                    ::quote::__private::push_ident(&mut _s, "self");
                    ::quote::__private::push_comma(&mut _s);
                    {
                        use ::quote::__private::ext::*;
                        let mut _i = 0usize;
                        let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                        #[allow(unused_mut)]
                        let (mut field_idents, i) = field_idents.quote_into_iter();
                        let has_iter = has_iter | i;
                        #[allow(unused_mut)]
                        let (mut field_types, i) = field_types.quote_into_iter();
                        let has_iter = has_iter | i;
                        let _: ::quote::__private::HasIterator = has_iter;
                        while true {
                            let field_idents = match field_idents.next() {
                                Some(_x) => ::quote::__private::RepInterp(_x),
                                None => break,
                            };
                            let field_types = match field_types.next() {
                                Some(_x) => ::quote::__private::RepInterp(_x),
                                None => break,
                            };
                            if _i > 0 {
                                ::quote::__private::push_comma(&mut _s);
                            }
                            _i += 1;
                            ::quote::ToTokens::to_tokens(&field_idents, &mut _s);
                            ::quote::__private::push_colon(&mut _s);
                            ::quote::ToTokens::to_tokens(&field_types, &mut _s);
                        }
                    }
                    _s
                },
            );
            ::quote::__private::push_rarrow(&mut _s);
            ::quote::__private::push_ident(&mut _s, "core");
            ::quote::__private::push_colon2(&mut _s);
            ::quote::__private::push_ident(&mut _s, "result");
            ::quote::__private::push_colon2(&mut _s);
            ::quote::__private::push_ident(&mut _s, "Result");
            ::quote::__private::push_lt(&mut _s);
            ::quote::ToTokens::to_tokens(&wrapper_enum_ident, &mut _s);
            ::quote::__private::push_comma(&mut _s);
            ::quote::__private::push_ident(&mut _s, "statum");
            ::quote::__private::push_colon2(&mut _s);
            ::quote::__private::push_ident(&mut _s, "Error");
            ::quote::__private::push_gt(&mut _s);
            _s
        }
    } else {
        {
            let mut _s = ::quote::__private::TokenStream::new();
            ::quote::__private::push_ident(&mut _s, "pub");
            ::quote::__private::push_ident(&mut _s, "fn");
            ::quote::__private::push_ident(&mut _s, "to_machine");
            ::quote::__private::push_group(
                &mut _s,
                ::quote::__private::Delimiter::Parenthesis,
                {
                    let mut _s = ::quote::__private::TokenStream::new();
                    ::quote::__private::push_and(&mut _s);
                    ::quote::__private::push_ident(&mut _s, "self");
                    ::quote::__private::push_comma(&mut _s);
                    {
                        use ::quote::__private::ext::*;
                        let mut _i = 0usize;
                        let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                        #[allow(unused_mut)]
                        let (mut field_idents, i) = field_idents.quote_into_iter();
                        let has_iter = has_iter | i;
                        #[allow(unused_mut)]
                        let (mut field_types, i) = field_types.quote_into_iter();
                        let has_iter = has_iter | i;
                        let _: ::quote::__private::HasIterator = has_iter;
                        while true {
                            let field_idents = match field_idents.next() {
                                Some(_x) => ::quote::__private::RepInterp(_x),
                                None => break,
                            };
                            let field_types = match field_types.next() {
                                Some(_x) => ::quote::__private::RepInterp(_x),
                                None => break,
                            };
                            if _i > 0 {
                                ::quote::__private::push_comma(&mut _s);
                            }
                            _i += 1;
                            ::quote::ToTokens::to_tokens(&field_idents, &mut _s);
                            ::quote::__private::push_colon(&mut _s);
                            ::quote::ToTokens::to_tokens(&field_types, &mut _s);
                        }
                    }
                    _s
                },
            );
            ::quote::__private::push_rarrow(&mut _s);
            ::quote::__private::push_ident(&mut _s, "core");
            ::quote::__private::push_colon2(&mut _s);
            ::quote::__private::push_ident(&mut _s, "result");
            ::quote::__private::push_colon2(&mut _s);
            ::quote::__private::push_ident(&mut _s, "Result");
            ::quote::__private::push_lt(&mut _s);
            ::quote::ToTokens::to_tokens(&wrapper_enum_ident, &mut _s);
            ::quote::__private::push_comma(&mut _s);
            ::quote::__private::push_ident(&mut _s, "statum");
            ::quote::__private::push_colon2(&mut _s);
            ::quote::__private::push_ident(&mut _s, "Error");
            ::quote::__private::push_gt(&mut _s);
            _s
        }
    };
    let try_methods = enum_variants
        .iter()
        .map(|variant| {
            let variant_ident = match ::quote::__private::IdentFragmentAdapter(
                &variant.name,
            ) {
                arg => {
                    ::quote::__private::mk_ident(
                        &::alloc::__export::must_use({
                            let res = ::alloc::fmt::format(format_args!("{0}", arg));
                            res
                        }),
                        ::quote::__private::Option::None.or(arg.span()),
                    )
                }
            };
            let try_method_name = match ::quote::__private::IdentFragmentAdapter(
                &to_snake_case(&variant.name),
            ) {
                arg => {
                    ::quote::__private::mk_ident(
                        &::alloc::__export::must_use({
                            let res = ::alloc::fmt::format(
                                format_args!("try_to_{0}", arg),
                            );
                            res
                        }),
                        ::quote::__private::Option::None.or(arg.span()),
                    )
                }
            };
            let is_method_name = match ::quote::__private::IdentFragmentAdapter(
                &to_snake_case(&variant.name),
            ) {
                arg => {
                    ::quote::__private::mk_ident(
                        &::alloc::__export::must_use({
                            let res = ::alloc::fmt::format(format_args!("is_{0}", arg));
                            res
                        }),
                        ::quote::__private::Option::None.or(arg.span()),
                    )
                }
            };
            let is_async = is_fns
                .iter()
                .any(|func| {
                    func.sig.ident == is_method_name && func.sig.asyncness.is_some()
                });
            if is_async {
                {
                    let mut _s = ::quote::__private::TokenStream::new();
                    ::quote::__private::push_ident(&mut _s, "pub");
                    ::quote::__private::push_ident(&mut _s, "async");
                    ::quote::__private::push_ident(&mut _s, "fn");
                    ::quote::ToTokens::to_tokens(&try_method_name, &mut _s);
                    ::quote::__private::push_group(
                        &mut _s,
                        ::quote::__private::Delimiter::Parenthesis,
                        {
                            let mut _s = ::quote::__private::TokenStream::new();
                            ::quote::__private::push_and(&mut _s);
                            ::quote::__private::push_ident(&mut _s, "self");
                            ::quote::__private::push_comma(&mut _s);
                            {
                                use ::quote::__private::ext::*;
                                let mut _i = 0usize;
                                let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                                #[allow(unused_mut)]
                                let (mut field_idents, i) = field_idents.quote_into_iter();
                                let has_iter = has_iter | i;
                                #[allow(unused_mut)]
                                let (mut field_types, i) = field_types.quote_into_iter();
                                let has_iter = has_iter | i;
                                let _: ::quote::__private::HasIterator = has_iter;
                                while true {
                                    let field_idents = match field_idents.next() {
                                        Some(_x) => ::quote::__private::RepInterp(_x),
                                        None => break,
                                    };
                                    let field_types = match field_types.next() {
                                        Some(_x) => ::quote::__private::RepInterp(_x),
                                        None => break,
                                    };
                                    if _i > 0 {
                                        ::quote::__private::push_comma(&mut _s);
                                    }
                                    _i += 1;
                                    ::quote::ToTokens::to_tokens(&field_idents, &mut _s);
                                    ::quote::__private::push_colon(&mut _s);
                                    ::quote::ToTokens::to_tokens(&field_types, &mut _s);
                                }
                            }
                            _s
                        },
                    );
                    ::quote::__private::push_rarrow(&mut _s);
                    ::quote::__private::push_ident(&mut _s, "core");
                    ::quote::__private::push_colon2(&mut _s);
                    ::quote::__private::push_ident(&mut _s, "result");
                    ::quote::__private::push_colon2(&mut _s);
                    ::quote::__private::push_ident(&mut _s, "Result");
                    ::quote::__private::push_lt(&mut _s);
                    ::quote::ToTokens::to_tokens(&machine_ident, &mut _s);
                    ::quote::__private::push_lt(&mut _s);
                    ::quote::ToTokens::to_tokens(&variant_ident, &mut _s);
                    ::quote::__private::push_gt(&mut _s);
                    ::quote::__private::push_comma(&mut _s);
                    ::quote::__private::push_ident(&mut _s, "statum");
                    ::quote::__private::push_colon2(&mut _s);
                    ::quote::__private::push_ident(&mut _s, "Error");
                    ::quote::__private::push_gt(&mut _s);
                    ::quote::__private::push_group(
                        &mut _s,
                        ::quote::__private::Delimiter::Brace,
                        {
                            let mut _s = ::quote::__private::TokenStream::new();
                            ::quote::__private::push_ident(&mut _s, "if");
                            ::quote::__private::push_ident(&mut _s, "self");
                            ::quote::__private::push_dot(&mut _s);
                            ::quote::ToTokens::to_tokens(&is_method_name, &mut _s);
                            ::quote::__private::push_group(
                                &mut _s,
                                ::quote::__private::Delimiter::Parenthesis,
                                {
                                    let mut _s = ::quote::__private::TokenStream::new();
                                    {
                                        use ::quote::__private::ext::*;
                                        let mut _i = 0usize;
                                        let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                                        #[allow(unused_mut)]
                                        let (mut field_idents, i) = field_idents.quote_into_iter();
                                        let has_iter = has_iter | i;
                                        let _: ::quote::__private::HasIterator = has_iter;
                                        while true {
                                            let field_idents = match field_idents.next() {
                                                Some(_x) => ::quote::__private::RepInterp(_x),
                                                None => break,
                                            };
                                            if _i > 0 {
                                                ::quote::__private::push_comma(&mut _s);
                                            }
                                            _i += 1;
                                            ::quote::__private::push_and(&mut _s);
                                            ::quote::ToTokens::to_tokens(&field_idents, &mut _s);
                                        }
                                    }
                                    _s
                                },
                            );
                            ::quote::__private::push_dot(&mut _s);
                            ::quote::__private::push_ident(&mut _s, "await");
                            ::quote::__private::push_dot(&mut _s);
                            ::quote::__private::push_ident(&mut _s, "is_ok");
                            ::quote::__private::push_group(
                                &mut _s,
                                ::quote::__private::Delimiter::Parenthesis,
                                ::quote::__private::TokenStream::new(),
                            );
                            ::quote::__private::push_group(
                                &mut _s,
                                ::quote::__private::Delimiter::Brace,
                                {
                                    let mut _s = ::quote::__private::TokenStream::new();
                                    ::quote::__private::push_ident(&mut _s, "Ok");
                                    ::quote::__private::push_group(
                                        &mut _s,
                                        ::quote::__private::Delimiter::Parenthesis,
                                        {
                                            let mut _s = ::quote::__private::TokenStream::new();
                                            ::quote::ToTokens::to_tokens(&machine_ident, &mut _s);
                                            ::quote::__private::push_colon2(&mut _s);
                                            ::quote::__private::push_lt(&mut _s);
                                            ::quote::ToTokens::to_tokens(&variant_ident, &mut _s);
                                            ::quote::__private::push_gt(&mut _s);
                                            ::quote::__private::push_colon2(&mut _s);
                                            ::quote::__private::push_ident(&mut _s, "new");
                                            ::quote::__private::push_group(
                                                &mut _s,
                                                ::quote::__private::Delimiter::Parenthesis,
                                                {
                                                    let mut _s = ::quote::__private::TokenStream::new();
                                                    {
                                                        use ::quote::__private::ext::*;
                                                        let mut _i = 0usize;
                                                        let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                                                        #[allow(unused_mut)]
                                                        let (mut field_idents, i) = field_idents.quote_into_iter();
                                                        let has_iter = has_iter | i;
                                                        let _: ::quote::__private::HasIterator = has_iter;
                                                        while true {
                                                            let field_idents = match field_idents.next() {
                                                                Some(_x) => ::quote::__private::RepInterp(_x),
                                                                None => break,
                                                            };
                                                            if _i > 0 {
                                                                ::quote::__private::push_comma(&mut _s);
                                                            }
                                                            _i += 1;
                                                            ::quote::ToTokens::to_tokens(&field_idents, &mut _s);
                                                        }
                                                    }
                                                    _s
                                                },
                                            );
                                            _s
                                        },
                                    );
                                    _s
                                },
                            );
                            ::quote::__private::push_ident(&mut _s, "else");
                            ::quote::__private::push_group(
                                &mut _s,
                                ::quote::__private::Delimiter::Brace,
                                {
                                    let mut _s = ::quote::__private::TokenStream::new();
                                    ::quote::__private::push_ident(&mut _s, "Err");
                                    ::quote::__private::push_group(
                                        &mut _s,
                                        ::quote::__private::Delimiter::Parenthesis,
                                        {
                                            let mut _s = ::quote::__private::TokenStream::new();
                                            ::quote::__private::push_ident(&mut _s, "statum");
                                            ::quote::__private::push_colon2(&mut _s);
                                            ::quote::__private::push_ident(&mut _s, "Error");
                                            ::quote::__private::push_colon2(&mut _s);
                                            ::quote::__private::push_ident(&mut _s, "InvalidState");
                                            _s
                                        },
                                    );
                                    _s
                                },
                            );
                            _s
                        },
                    );
                    _s
                }
            } else {
                {
                    let mut _s = ::quote::__private::TokenStream::new();
                    ::quote::__private::push_ident(&mut _s, "pub");
                    ::quote::__private::push_ident(&mut _s, "fn");
                    ::quote::ToTokens::to_tokens(&try_method_name, &mut _s);
                    ::quote::__private::push_group(
                        &mut _s,
                        ::quote::__private::Delimiter::Parenthesis,
                        {
                            let mut _s = ::quote::__private::TokenStream::new();
                            ::quote::__private::push_and(&mut _s);
                            ::quote::__private::push_ident(&mut _s, "self");
                            ::quote::__private::push_comma(&mut _s);
                            {
                                use ::quote::__private::ext::*;
                                let mut _i = 0usize;
                                let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                                #[allow(unused_mut)]
                                let (mut field_idents, i) = field_idents.quote_into_iter();
                                let has_iter = has_iter | i;
                                #[allow(unused_mut)]
                                let (mut field_types, i) = field_types.quote_into_iter();
                                let has_iter = has_iter | i;
                                let _: ::quote::__private::HasIterator = has_iter;
                                while true {
                                    let field_idents = match field_idents.next() {
                                        Some(_x) => ::quote::__private::RepInterp(_x),
                                        None => break,
                                    };
                                    let field_types = match field_types.next() {
                                        Some(_x) => ::quote::__private::RepInterp(_x),
                                        None => break,
                                    };
                                    if _i > 0 {
                                        ::quote::__private::push_comma(&mut _s);
                                    }
                                    _i += 1;
                                    ::quote::ToTokens::to_tokens(&field_idents, &mut _s);
                                    ::quote::__private::push_colon(&mut _s);
                                    ::quote::ToTokens::to_tokens(&field_types, &mut _s);
                                }
                            }
                            _s
                        },
                    );
                    ::quote::__private::push_rarrow(&mut _s);
                    ::quote::__private::push_ident(&mut _s, "core");
                    ::quote::__private::push_colon2(&mut _s);
                    ::quote::__private::push_ident(&mut _s, "result");
                    ::quote::__private::push_colon2(&mut _s);
                    ::quote::__private::push_ident(&mut _s, "Result");
                    ::quote::__private::push_lt(&mut _s);
                    ::quote::ToTokens::to_tokens(&machine_ident, &mut _s);
                    ::quote::__private::push_lt(&mut _s);
                    ::quote::ToTokens::to_tokens(&variant_ident, &mut _s);
                    ::quote::__private::push_gt(&mut _s);
                    ::quote::__private::push_comma(&mut _s);
                    ::quote::__private::push_ident(&mut _s, "statum");
                    ::quote::__private::push_colon2(&mut _s);
                    ::quote::__private::push_ident(&mut _s, "Error");
                    ::quote::__private::push_gt(&mut _s);
                    ::quote::__private::push_group(
                        &mut _s,
                        ::quote::__private::Delimiter::Brace,
                        {
                            let mut _s = ::quote::__private::TokenStream::new();
                            ::quote::__private::push_ident(&mut _s, "if");
                            ::quote::__private::push_ident(&mut _s, "self");
                            ::quote::__private::push_dot(&mut _s);
                            ::quote::ToTokens::to_tokens(&is_method_name, &mut _s);
                            ::quote::__private::push_group(
                                &mut _s,
                                ::quote::__private::Delimiter::Parenthesis,
                                {
                                    let mut _s = ::quote::__private::TokenStream::new();
                                    {
                                        use ::quote::__private::ext::*;
                                        let mut _i = 0usize;
                                        let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                                        #[allow(unused_mut)]
                                        let (mut field_idents, i) = field_idents.quote_into_iter();
                                        let has_iter = has_iter | i;
                                        let _: ::quote::__private::HasIterator = has_iter;
                                        while true {
                                            let field_idents = match field_idents.next() {
                                                Some(_x) => ::quote::__private::RepInterp(_x),
                                                None => break,
                                            };
                                            if _i > 0 {
                                                ::quote::__private::push_comma(&mut _s);
                                            }
                                            _i += 1;
                                            ::quote::__private::push_and(&mut _s);
                                            ::quote::ToTokens::to_tokens(&field_idents, &mut _s);
                                        }
                                    }
                                    _s
                                },
                            );
                            ::quote::__private::push_dot(&mut _s);
                            ::quote::__private::push_ident(&mut _s, "is_ok");
                            ::quote::__private::push_group(
                                &mut _s,
                                ::quote::__private::Delimiter::Parenthesis,
                                ::quote::__private::TokenStream::new(),
                            );
                            ::quote::__private::push_group(
                                &mut _s,
                                ::quote::__private::Delimiter::Brace,
                                {
                                    let mut _s = ::quote::__private::TokenStream::new();
                                    ::quote::__private::push_ident(&mut _s, "Ok");
                                    ::quote::__private::push_group(
                                        &mut _s,
                                        ::quote::__private::Delimiter::Parenthesis,
                                        {
                                            let mut _s = ::quote::__private::TokenStream::new();
                                            ::quote::ToTokens::to_tokens(&machine_ident, &mut _s);
                                            ::quote::__private::push_colon2(&mut _s);
                                            ::quote::__private::push_lt(&mut _s);
                                            ::quote::ToTokens::to_tokens(&variant_ident, &mut _s);
                                            ::quote::__private::push_gt(&mut _s);
                                            ::quote::__private::push_colon2(&mut _s);
                                            ::quote::__private::push_ident(&mut _s, "new");
                                            ::quote::__private::push_group(
                                                &mut _s,
                                                ::quote::__private::Delimiter::Parenthesis,
                                                {
                                                    let mut _s = ::quote::__private::TokenStream::new();
                                                    {
                                                        use ::quote::__private::ext::*;
                                                        let mut _i = 0usize;
                                                        let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                                                        #[allow(unused_mut)]
                                                        let (mut field_idents, i) = field_idents.quote_into_iter();
                                                        let has_iter = has_iter | i;
                                                        let _: ::quote::__private::HasIterator = has_iter;
                                                        while true {
                                                            let field_idents = match field_idents.next() {
                                                                Some(_x) => ::quote::__private::RepInterp(_x),
                                                                None => break,
                                                            };
                                                            if _i > 0 {
                                                                ::quote::__private::push_comma(&mut _s);
                                                            }
                                                            _i += 1;
                                                            ::quote::ToTokens::to_tokens(&field_idents, &mut _s);
                                                        }
                                                    }
                                                    _s
                                                },
                                            );
                                            _s
                                        },
                                    );
                                    _s
                                },
                            );
                            ::quote::__private::push_ident(&mut _s, "else");
                            ::quote::__private::push_group(
                                &mut _s,
                                ::quote::__private::Delimiter::Brace,
                                {
                                    let mut _s = ::quote::__private::TokenStream::new();
                                    ::quote::__private::push_ident(&mut _s, "Err");
                                    ::quote::__private::push_group(
                                        &mut _s,
                                        ::quote::__private::Delimiter::Parenthesis,
                                        {
                                            let mut _s = ::quote::__private::TokenStream::new();
                                            ::quote::__private::push_ident(&mut _s, "statum");
                                            ::quote::__private::push_colon2(&mut _s);
                                            ::quote::__private::push_ident(&mut _s, "Error");
                                            ::quote::__private::push_colon2(&mut _s);
                                            ::quote::__private::push_ident(&mut _s, "InvalidState");
                                            _s
                                        },
                                    );
                                    _s
                                },
                            );
                            _s
                        },
                    );
                    _s
                }
            }
        });
    let modified_impl_items: Vec<proc_macro2::TokenStream> = impl_block
        .items
        .iter()
        .map(|item| {
            if let syn::ImplItem::Fn(method) = item {
                if method.sig.ident.to_string().starts_with("is_") {
                    let sig = &method.sig;
                    let method_name = &sig.ident;
                    let mut updated_inputs: Vec<syn::FnArg> = sig
                        .inputs
                        .iter()
                        .cloned()
                        .collect();
                    for (field_name, field_type) in &fields {
                        let field_ident = syn::parse_str::<syn::Ident>(field_name)
                            .unwrap_or_else(|_| {
                                {
                                    ::core::panicking::panic_fmt(
                                        format_args!("Failed to parse \'{0}\' as Ident", field_name),
                                    );
                                }
                            });
                        let field_ty = if field_type == "String" {
                            syn::parse_str::<syn::Type>("str").unwrap()
                        } else {
                            syn::parse_str::<syn::Type>(field_type).unwrap()
                        };
                        updated_inputs
                            .push(
                                syn::FnArg::Typed(
                                    ::syn::__private::parse_quote({
                                        let mut _s = ::quote::__private::TokenStream::new();
                                        ::quote::ToTokens::to_tokens(&field_ident, &mut _s);
                                        ::quote::__private::push_colon(&mut _s);
                                        ::quote::__private::push_and(&mut _s);
                                        ::quote::ToTokens::to_tokens(&field_ty, &mut _s);
                                        _s
                                    }),
                                ),
                            );
                    }
                    let method_body = &method.block;
                    let asyncness = &sig.asyncness;
                    let output = &sig.output;
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::__private::push_ident(&mut _s, "pub");
                        ::quote::ToTokens::to_tokens(&asyncness, &mut _s);
                        ::quote::__private::push_ident(&mut _s, "fn");
                        ::quote::ToTokens::to_tokens(&method_name, &mut _s);
                        ::quote::__private::push_group(
                            &mut _s,
                            ::quote::__private::Delimiter::Parenthesis,
                            {
                                let mut _s = ::quote::__private::TokenStream::new();
                                {
                                    use ::quote::__private::ext::*;
                                    let mut _i = 0usize;
                                    let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                                    #[allow(unused_mut)]
                                    let (mut updated_inputs, i) = updated_inputs
                                        .quote_into_iter();
                                    let has_iter = has_iter | i;
                                    let _: ::quote::__private::HasIterator = has_iter;
                                    while true {
                                        let updated_inputs = match updated_inputs.next() {
                                            Some(_x) => ::quote::__private::RepInterp(_x),
                                            None => break,
                                        };
                                        if _i > 0 {
                                            ::quote::__private::push_comma(&mut _s);
                                        }
                                        _i += 1;
                                        ::quote::ToTokens::to_tokens(&updated_inputs, &mut _s);
                                    }
                                }
                                _s
                            },
                        );
                        ::quote::ToTokens::to_tokens(&output, &mut _s);
                        ::quote::__private::push_group(
                            &mut _s,
                            ::quote::__private::Delimiter::Brace,
                            {
                                let mut _s = ::quote::__private::TokenStream::new();
                                ::quote::ToTokens::to_tokens(&method_body, &mut _s);
                                _s
                            },
                        );
                        _s
                    }
                } else {
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::ToTokens::to_tokens(&item, &mut _s);
                        _s
                    }
                }
            } else {
                {
                    let mut _s = ::quote::__private::TokenStream::new();
                    ::quote::ToTokens::to_tokens(&item, &mut _s);
                    _s
                }
            }
        })
        .collect();
    let modified_impl_block = {
        let mut _s = ::quote::__private::TokenStream::new();
        ::quote::__private::push_ident(&mut _s, "impl");
        ::quote::ToTokens::to_tokens(&self_ty, &mut _s);
        ::quote::__private::push_group(
            &mut _s,
            ::quote::__private::Delimiter::Brace,
            {
                let mut _s = ::quote::__private::TokenStream::new();
                {
                    use ::quote::__private::ext::*;
                    let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                    #[allow(unused_mut)]
                    let (mut modified_impl_items, i) = modified_impl_items
                        .quote_into_iter();
                    let has_iter = has_iter | i;
                    let _: ::quote::__private::HasIterator = has_iter;
                    while true {
                        let modified_impl_items = match modified_impl_items.next() {
                            Some(_x) => ::quote::__private::RepInterp(_x),
                            None => break,
                        };
                        ::quote::ToTokens::to_tokens(&modified_impl_items, &mut _s);
                    }
                }
                _s
            },
        );
        _s
    };
    let expanded = {
        let mut _s = ::quote::__private::TokenStream::new();
        ::quote::ToTokens::to_tokens(&wrapper_enum, &mut _s);
        ::quote::ToTokens::to_tokens(&modified_impl_block, &mut _s);
        ::quote::__private::push_ident(&mut _s, "impl");
        ::quote::ToTokens::to_tokens(&self_ty, &mut _s);
        ::quote::__private::push_group(
            &mut _s,
            ::quote::__private::Delimiter::Brace,
            {
                let mut _s = ::quote::__private::TokenStream::new();
                ::quote::ToTokens::to_tokens(&to_machine_signature, &mut _s);
                ::quote::__private::push_group(
                    &mut _s,
                    ::quote::__private::Delimiter::Brace,
                    {
                        let mut _s = ::quote::__private::TokenStream::new();
                        ::quote::ToTokens::to_tokens(&to_machine_checks, &mut _s);
                        _s
                    },
                );
                _s
            },
        );
        ::quote::__private::push_ident(&mut _s, "impl");
        ::quote::ToTokens::to_tokens(&self_ty, &mut _s);
        ::quote::__private::push_group(
            &mut _s,
            ::quote::__private::Delimiter::Brace,
            {
                let mut _s = ::quote::__private::TokenStream::new();
                {
                    use ::quote::__private::ext::*;
                    let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                    #[allow(unused_mut)]
                    let (mut try_methods, i) = try_methods.quote_into_iter();
                    let has_iter = has_iter | i;
                    let _: ::quote::__private::HasIterator = has_iter;
                    while true {
                        let try_methods = match try_methods.next() {
                            Some(_x) => ::quote::__private::RepInterp(_x),
                            None => break,
                        };
                        ::quote::ToTokens::to_tokens(&try_methods, &mut _s);
                    }
                }
                _s
            },
        );
        _s
    };
    expanded.into()
}
fn parse_validators_attr(attr: TokenStream) -> syn::Result<(syn::Ident, syn::Ident)> {
    let parsed = syn::parse::<ValidatorsAttr>(attr)?;
    Ok((parsed.state, parsed.machine))
}
fn get_variants_of_state(state_ident: &syn::Ident) -> syn::Result<Vec<VariantInfo>> {
    let enum_name = state_ident.to_string();
    match get_state_variants(&enum_name) {
        Some(variants) => Ok(variants),
        None => {
            Err(
                syn::Error::new_spanned(
                    state_ident,
                    ::alloc::__export::must_use({
                        let res = ::alloc::fmt::format(
                            format_args!(
                                "No variants found for enum `{0}`. Did you mark it with #[state]?",
                                enum_name,
                            ),
                        );
                        res
                    }),
                ),
            )
        }
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
            .map(|s| match ::quote::__private::IdentFragmentAdapter(&s.0) {
                arg => {
                    ::quote::__private::mk_ident(
                        &::alloc::__export::must_use({
                            let res = ::alloc::fmt::format(format_args!("{0}", arg));
                            res
                        }),
                        ::quote::__private::Option::None.or(arg.span()),
                    )
                }
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let mut checks = ::alloc::vec::Vec::new();
    let mut has_async = false;
    for variant_info in enum_variants {
        let variant = &variant_info.name;
        let variant_snake = to_snake_case(variant);
        let is_method_ident = match ::quote::__private::IdentFragmentAdapter(
            &variant_snake,
        ) {
            arg => {
                ::quote::__private::mk_ident(
                    &::alloc::__export::must_use({
                        let res = ::alloc::fmt::format(format_args!("is_{0}", arg));
                        res
                    }),
                    ::quote::__private::Option::None.or(arg.span()),
                )
            }
        };
        let variant_ident = match ::quote::__private::IdentFragmentAdapter(&variant) {
            arg => {
                ::quote::__private::mk_ident(
                    &::alloc::__export::must_use({
                        let res = ::alloc::fmt::format(format_args!("{0}", arg));
                        res
                    }),
                    ::quote::__private::Option::None.or(arg.span()),
                )
            }
        };
        let user_fn = is_fns.iter().find(|f| f.sig.ident == is_method_ident);
        if let Some(f) = user_fn {
            let is_async = f.sig.asyncness.is_some();
            if is_async {
                has_async = true;
            }
            let await_token = if is_async {
                {
                    let mut _s = ::quote::__private::TokenStream::new();
                    ::quote::__private::push_dot(&mut _s);
                    ::quote::__private::push_ident(&mut _s, "await");
                    _s
                }
            } else {
                ::quote::__private::TokenStream::new()
            };
            if let Some((ok_ty_opt, _err_ty_opt)) = extract_result_ok_err_types(
                &f.sig.output,
            ) {
                let expects_data = variant_info.data_type.is_some();
                match (expects_data, ok_ty_opt) {
                    (true, Some(_ty)) => {
                        checks
                            .push({
                                let mut _s = ::quote::__private::TokenStream::new();
                                ::quote::__private::push_ident(&mut _s, "if");
                                ::quote::__private::push_ident(&mut _s, "let");
                                ::quote::__private::push_ident(&mut _s, "Ok");
                                ::quote::__private::push_group(
                                    &mut _s,
                                    ::quote::__private::Delimiter::Parenthesis,
                                    {
                                        let mut _s = ::quote::__private::TokenStream::new();
                                        ::quote::__private::push_ident(&mut _s, "data");
                                        _s
                                    },
                                );
                                ::quote::__private::push_eq(&mut _s);
                                ::quote::__private::push_ident(&mut _s, "self");
                                ::quote::__private::push_dot(&mut _s);
                                ::quote::ToTokens::to_tokens(&is_method_ident, &mut _s);
                                ::quote::__private::push_group(
                                    &mut _s,
                                    ::quote::__private::Delimiter::Parenthesis,
                                    {
                                        let mut _s = ::quote::__private::TokenStream::new();
                                        {
                                            use ::quote::__private::ext::*;
                                            let mut _i = 0usize;
                                            let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                                            #[allow(unused_mut)]
                                            let (mut field_idents, i) = field_idents.quote_into_iter();
                                            let has_iter = has_iter | i;
                                            let _: ::quote::__private::HasIterator = has_iter;
                                            while true {
                                                let field_idents = match field_idents.next() {
                                                    Some(_x) => ::quote::__private::RepInterp(_x),
                                                    None => break,
                                                };
                                                if _i > 0 {
                                                    ::quote::__private::push_comma(&mut _s);
                                                }
                                                _i += 1;
                                                ::quote::__private::push_and(&mut _s);
                                                ::quote::ToTokens::to_tokens(&field_idents, &mut _s);
                                            }
                                        }
                                        _s
                                    },
                                );
                                ::quote::ToTokens::to_tokens(&await_token, &mut _s);
                                ::quote::__private::push_group(
                                    &mut _s,
                                    ::quote::__private::Delimiter::Brace,
                                    {
                                        let mut _s = ::quote::__private::TokenStream::new();
                                        ::quote::__private::push_ident(&mut _s, "let");
                                        ::quote::__private::push_ident(&mut _s, "machine");
                                        ::quote::__private::push_eq(&mut _s);
                                        ::quote::ToTokens::to_tokens(&machine_ident, &mut _s);
                                        ::quote::__private::push_colon2(&mut _s);
                                        ::quote::__private::push_lt(&mut _s);
                                        ::quote::ToTokens::to_tokens(&variant_ident, &mut _s);
                                        ::quote::__private::push_gt(&mut _s);
                                        ::quote::__private::push_colon2(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "new");
                                        ::quote::__private::push_group(
                                            &mut _s,
                                            ::quote::__private::Delimiter::Parenthesis,
                                            {
                                                let mut _s = ::quote::__private::TokenStream::new();
                                                {
                                                    use ::quote::__private::ext::*;
                                                    let mut _i = 0usize;
                                                    let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                                                    #[allow(unused_mut)]
                                                    let (mut field_idents, i) = field_idents.quote_into_iter();
                                                    let has_iter = has_iter | i;
                                                    let _: ::quote::__private::HasIterator = has_iter;
                                                    while true {
                                                        let field_idents = match field_idents.next() {
                                                            Some(_x) => ::quote::__private::RepInterp(_x),
                                                            None => break,
                                                        };
                                                        if _i > 0 {
                                                            ::quote::__private::push_comma(&mut _s);
                                                        }
                                                        _i += 1;
                                                        ::quote::ToTokens::to_tokens(&field_idents, &mut _s);
                                                        ::quote::__private::push_dot(&mut _s);
                                                        ::quote::__private::push_ident(&mut _s, "clone");
                                                        ::quote::__private::push_group(
                                                            &mut _s,
                                                            ::quote::__private::Delimiter::Parenthesis,
                                                            ::quote::__private::TokenStream::new(),
                                                        );
                                                    }
                                                }
                                                _s
                                            },
                                        );
                                        ::quote::__private::push_dot(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "transition_with");
                                        ::quote::__private::push_group(
                                            &mut _s,
                                            ::quote::__private::Delimiter::Parenthesis,
                                            {
                                                let mut _s = ::quote::__private::TokenStream::new();
                                                ::quote::__private::push_ident(&mut _s, "data");
                                                _s
                                            },
                                        );
                                        ::quote::__private::push_semi(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "return");
                                        ::quote::__private::push_ident(&mut _s, "Ok");
                                        ::quote::__private::push_group(
                                            &mut _s,
                                            ::quote::__private::Delimiter::Parenthesis,
                                            {
                                                let mut _s = ::quote::__private::TokenStream::new();
                                                ::quote::ToTokens::to_tokens(&wrapper_enum_ident, &mut _s);
                                                ::quote::__private::push_colon2(&mut _s);
                                                ::quote::ToTokens::to_tokens(&variant_ident, &mut _s);
                                                ::quote::__private::push_group(
                                                    &mut _s,
                                                    ::quote::__private::Delimiter::Parenthesis,
                                                    {
                                                        let mut _s = ::quote::__private::TokenStream::new();
                                                        ::quote::__private::push_ident(&mut _s, "machine");
                                                        _s
                                                    },
                                                );
                                                _s
                                            },
                                        );
                                        ::quote::__private::push_semi(&mut _s);
                                        _s
                                    },
                                );
                                _s
                            });
                    }
                    (false, Some(Type::Tuple(t))) if t.elems.is_empty() => {
                        checks
                            .push({
                                let mut _s = ::quote::__private::TokenStream::new();
                                ::quote::__private::push_ident(&mut _s, "if");
                                ::quote::__private::push_ident(&mut _s, "let");
                                ::quote::__private::push_ident(&mut _s, "Ok");
                                ::quote::__private::push_group(
                                    &mut _s,
                                    ::quote::__private::Delimiter::Parenthesis,
                                    {
                                        let mut _s = ::quote::__private::TokenStream::new();
                                        ::quote::__private::push_group(
                                            &mut _s,
                                            ::quote::__private::Delimiter::Parenthesis,
                                            ::quote::__private::TokenStream::new(),
                                        );
                                        _s
                                    },
                                );
                                ::quote::__private::push_eq(&mut _s);
                                ::quote::__private::push_ident(&mut _s, "self");
                                ::quote::__private::push_dot(&mut _s);
                                ::quote::ToTokens::to_tokens(&is_method_ident, &mut _s);
                                ::quote::__private::push_group(
                                    &mut _s,
                                    ::quote::__private::Delimiter::Parenthesis,
                                    {
                                        let mut _s = ::quote::__private::TokenStream::new();
                                        {
                                            use ::quote::__private::ext::*;
                                            let mut _i = 0usize;
                                            let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                                            #[allow(unused_mut)]
                                            let (mut field_idents, i) = field_idents.quote_into_iter();
                                            let has_iter = has_iter | i;
                                            let _: ::quote::__private::HasIterator = has_iter;
                                            while true {
                                                let field_idents = match field_idents.next() {
                                                    Some(_x) => ::quote::__private::RepInterp(_x),
                                                    None => break,
                                                };
                                                if _i > 0 {
                                                    ::quote::__private::push_comma(&mut _s);
                                                }
                                                _i += 1;
                                                ::quote::__private::push_and(&mut _s);
                                                ::quote::ToTokens::to_tokens(&field_idents, &mut _s);
                                            }
                                        }
                                        _s
                                    },
                                );
                                ::quote::ToTokens::to_tokens(&await_token, &mut _s);
                                ::quote::__private::push_group(
                                    &mut _s,
                                    ::quote::__private::Delimiter::Brace,
                                    {
                                        let mut _s = ::quote::__private::TokenStream::new();
                                        ::quote::__private::push_ident(&mut _s, "let");
                                        ::quote::__private::push_ident(&mut _s, "machine");
                                        ::quote::__private::push_eq(&mut _s);
                                        ::quote::ToTokens::to_tokens(&machine_ident, &mut _s);
                                        ::quote::__private::push_colon2(&mut _s);
                                        ::quote::__private::push_lt(&mut _s);
                                        ::quote::ToTokens::to_tokens(&variant_ident, &mut _s);
                                        ::quote::__private::push_gt(&mut _s);
                                        ::quote::__private::push_colon2(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "new");
                                        ::quote::__private::push_group(
                                            &mut _s,
                                            ::quote::__private::Delimiter::Parenthesis,
                                            {
                                                let mut _s = ::quote::__private::TokenStream::new();
                                                {
                                                    use ::quote::__private::ext::*;
                                                    let mut _i = 0usize;
                                                    let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
                                                    #[allow(unused_mut)]
                                                    let (mut field_idents, i) = field_idents.quote_into_iter();
                                                    let has_iter = has_iter | i;
                                                    let _: ::quote::__private::HasIterator = has_iter;
                                                    while true {
                                                        let field_idents = match field_idents.next() {
                                                            Some(_x) => ::quote::__private::RepInterp(_x),
                                                            None => break,
                                                        };
                                                        if _i > 0 {
                                                            ::quote::__private::push_comma(&mut _s);
                                                        }
                                                        _i += 1;
                                                        ::quote::ToTokens::to_tokens(&field_idents, &mut _s);
                                                        ::quote::__private::push_dot(&mut _s);
                                                        ::quote::__private::push_ident(&mut _s, "clone");
                                                        ::quote::__private::push_group(
                                                            &mut _s,
                                                            ::quote::__private::Delimiter::Parenthesis,
                                                            ::quote::__private::TokenStream::new(),
                                                        );
                                                    }
                                                }
                                                _s
                                            },
                                        );
                                        ::quote::__private::push_semi(&mut _s);
                                        ::quote::__private::push_ident(&mut _s, "return");
                                        ::quote::__private::push_ident(&mut _s, "Ok");
                                        ::quote::__private::push_group(
                                            &mut _s,
                                            ::quote::__private::Delimiter::Parenthesis,
                                            {
                                                let mut _s = ::quote::__private::TokenStream::new();
                                                ::quote::ToTokens::to_tokens(&wrapper_enum_ident, &mut _s);
                                                ::quote::__private::push_colon2(&mut _s);
                                                ::quote::ToTokens::to_tokens(&variant_ident, &mut _s);
                                                ::quote::__private::push_group(
                                                    &mut _s,
                                                    ::quote::__private::Delimiter::Parenthesis,
                                                    {
                                                        let mut _s = ::quote::__private::TokenStream::new();
                                                        ::quote::__private::push_ident(&mut _s, "machine");
                                                        _s
                                                    },
                                                );
                                                _s
                                            },
                                        );
                                        ::quote::__private::push_semi(&mut _s);
                                        _s
                                    },
                                );
                                _s
                            });
                    }
                    _ => {
                        checks
                            .push(
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
            {
                ::std::io::_print(
                    format_args!("No user_fn for variant \'{0}\'\n", variant),
                );
            };
            checks
                .push(
                    syn::Error::new(
                            proc_macro2::Span::call_site(),
                            ::alloc::__export::must_use({
                                let res = ::alloc::fmt::format(
                                    format_args!(
                                        "Missing validator method for variant \'{0}\'",
                                        variant,
                                    ),
                                );
                                res
                            }),
                        )
                        .to_compile_error(),
                );
        }
    }
    let generated_checks = {
        let mut _s = ::quote::__private::TokenStream::new();
        {
            use ::quote::__private::ext::*;
            let has_iter = ::quote::__private::ThereIsNoIteratorInRepetition;
            #[allow(unused_mut)]
            let (mut checks, i) = checks.quote_into_iter();
            let has_iter = has_iter | i;
            let _: ::quote::__private::HasIterator = has_iter;
            while true {
                let checks = match checks.next() {
                    Some(_x) => ::quote::__private::RepInterp(_x),
                    None => break,
                };
                ::quote::ToTokens::to_tokens(&checks, &mut _s);
            }
        }
        ::quote::__private::push_ident(&mut _s, "Err");
        ::quote::__private::push_group(
            &mut _s,
            ::quote::__private::Delimiter::Parenthesis,
            {
                let mut _s = ::quote::__private::TokenStream::new();
                ::quote::__private::push_ident(&mut _s, "statum");
                ::quote::__private::push_colon2(&mut _s);
                ::quote::__private::push_ident(&mut _s, "Error");
                ::quote::__private::push_colon2(&mut _s);
                ::quote::__private::push_ident(&mut _s, "InvalidState");
                _s
            },
        );
        _s
    };
    (generated_checks, has_async)
}
/// Helper to parse something like `-> Result<T, E>`
/// Returns `Some((Some(T), Some(E)))` or `Some((Some(T), None))` etc.
/// If it doesn't match a `Result<_, _>` signature, returns `None`.
fn extract_result_ok_err_types(
    ret: &ReturnType,
) -> Option<(Option<Type>, Option<Type>)> {
    if let ReturnType::Type(_, ty) = ret {
        if let Type::Path(type_path) = &**ty {
            let segments = &type_path.path.segments;
            if let Some(seg) = segments.last() {
                if seg.ident == "Result" {
                    if let PathArguments::AngleBracketed(args) = &seg.arguments {
                        let args = &args.args;
                        if args.len() == 2 {
                            let mut iter = args.iter();
                            let first = iter.next().unwrap();
                            let second = iter.next().unwrap();
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
                        if args.len() == 1 {
                            let mut iter = args.iter();
                            let first = iter.next().unwrap();
                            let ok_ty = match first {
                                syn::GenericArgument::Type(t) => Some(t.clone()),
                                _ => None,
                            };
                            return Some((ok_ty, None));
                        } else {
                            {
                                ::core::panicking::panic_fmt(
                                    format_args!("Expected 1 or 2 arguments in Result<_, _>"),
                                );
                            };
                        }
                    }
                }
            }
        }
    }
    None
}
const _: () = {
    extern crate proc_macro;
    #[rustc_proc_macro_decls]
    #[used]
    #[allow(deprecated)]
    static _DECLS: &[proc_macro::bridge::client::ProcMacro] = &[
        proc_macro::bridge::client::ProcMacro::attr("state", state),
        proc_macro::bridge::client::ProcMacro::attr("machine", machine),
        proc_macro::bridge::client::ProcMacro::attr("model", model),
        proc_macro::bridge::client::ProcMacro::attr("validators", validators),
    ];
};
