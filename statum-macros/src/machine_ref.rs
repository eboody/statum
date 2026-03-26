use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::{Fields, Item, Type, parse_macro_input};

use crate::relation::parse_machine_reference_target;
use crate::{ItemTarget, resolved_current_module_path};

pub fn parse_machine_ref(attr: TokenStream, item: TokenStream) -> TokenStream {
    let target_type = parse_macro_input!(attr as Type);
    let item = parse_macro_input!(item as Item);
    let item_struct = match item {
        Item::Struct(item_struct) => item_struct,
        other => return invalid_machine_ref_target_error(&other).into(),
    };

    if !item_struct.generics.params.is_empty() {
        return syn::Error::new_spanned(
            &item_struct.generics,
            format!(
                "Error: `#[machine_ref(...)]` on `{}` does not support generics in v1.\nFix: declare a concrete nominal wrapper type and attach `#[machine_ref(...)]` there.",
                item_struct.ident
            ),
        )
        .to_compile_error()
        .into();
    }

    match &item_struct.fields {
        Fields::Named(_) | Fields::Unnamed(_) => {}
        Fields::Unit => {
            return syn::Error::new_spanned(
                &item_struct.fields,
                format!(
                    "Error: `#[machine_ref(...)]` on `{}` requires a nominal struct or tuple struct with stored data.\nFix: wrap the opaque reference value in a field and attach `#[machine_ref(...)]` to that struct.",
                    item_struct.ident
                ),
            )
            .to_compile_error()
            .into();
        }
    }

    let line_number = item_struct.ident.span().start().line;
    let module_path = match resolved_current_module_path(item_struct.ident.span(), "#[machine_ref]") {
        Ok(path) => path,
        Err(err) => return err,
    };
    let (machine_path, state_name) = match parse_machine_reference_target(&target_type, &module_path) {
        Ok(target) => target,
        Err(err) => return err.into(),
    };
    let rust_type_path = format!("{}::{}", module_path, item_struct.ident);
    let rust_type_path_lit = syn::LitStr::new(&rust_type_path, Span::call_site());
    let machine_path_tokens = machine_path.iter().map(|segment| {
        let segment = syn::LitStr::new(segment, Span::call_site());
        quote! { #segment }
    });
    let state_name_lit = syn::LitStr::new(&state_name, Span::call_site());
    let targets_ident = linked_reference_targets_ident(&rust_type_path, line_number);
    let type_name_ident = linked_reference_type_name_ident(&rust_type_path, line_number);
    let registration_ident = linked_reference_registration_ident(&rust_type_path, line_number);
    let item_ident = &item_struct.ident;

    quote! {
        #item_struct

        #[doc(hidden)]
        static #targets_ident: &[&str] = &[#(#machine_path_tokens),*];

        #[doc(hidden)]
        fn #type_name_ident() -> &'static str {
            ::core::any::type_name::<#item_ident>()
        }

        impl statum::MachineReference for #item_ident {
            const TARGET: statum::MachineReferenceTarget = statum::MachineReferenceTarget {
                machine_path: #targets_ident,
                state: #state_name_lit,
            };
        }

        #[doc(hidden)]
        #[statum::__private::linkme::distributed_slice(statum::__private::__STATUM_LINKED_REFERENCE_TYPES)]
        #[linkme(crate = statum::__private::linkme)]
        static #registration_ident: statum::__private::LinkedReferenceTypeDescriptor =
            statum::__private::LinkedReferenceTypeDescriptor {
                rust_type_path: #rust_type_path_lit,
                resolved_type_name: #type_name_ident,
                to_machine_path: <#item_ident as statum::MachineReference>::TARGET.machine_path,
                to_state: <#item_ident as statum::MachineReference>::TARGET.state,
            };
    }
    .into()
}

fn invalid_machine_ref_target_error(item: &Item) -> proc_macro2::TokenStream {
    let target = ItemTarget::from(item);
    let item_name = target
        .name()
        .map(|name| format!(" `{name}`"))
        .unwrap_or_default();
    let message = format!(
        "Error: `#[machine_ref(...)]` must be applied to a nominal struct or tuple struct, but this item is {} {}{}.\nFix: apply `#[machine_ref(...)]` to a nominal wrapper struct like `struct TaskId(Uuid);`.",
        target.article(),
        target.kind(),
        item_name,
    );
    quote! { compile_error!(#message); }
}

fn linked_reference_registration_ident(rust_type_path: &str, line_number: usize) -> syn::Ident {
    format_ident!(
        "__STATUM_LINKED_REFERENCE_TYPE_{:016X}",
        stable_hash(&format!("{rust_type_path}::{line_number}::reference"))
    )
}

fn linked_reference_targets_ident(rust_type_path: &str, line_number: usize) -> syn::Ident {
    format_ident!(
        "__STATUM_LINKED_REFERENCE_TARGET_{:016X}",
        stable_hash(&format!("{rust_type_path}::{line_number}::target"))
    )
}

fn linked_reference_type_name_ident(rust_type_path: &str, line_number: usize) -> syn::Ident {
    format_ident!(
        "__statum_machine_ref_type_name_{:016x}",
        stable_hash(&format!("{rust_type_path}::{line_number}::type_name"))
    )
}

fn stable_hash(input: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
