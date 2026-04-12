#![allow(unexpected_cfgs)]

//! Proc-macro implementation crate for Statum.
//!
//! Most users should depend on [`statum`](https://docs.rs/statum), which
//! re-exports these macros with the public-facing documentation. This crate
//! exists so the macro expansion logic can stay separate from the stable runtime
//! traits in `statum-core`.
//!
//! The public macros are:
//!
//! - [`state`] for declaring legal lifecycle phases
//! - [`machine`] for declaring the typed machine and durable context
//! - [`machine_ref`] for declaring one nominal opaque machine reference type
//! - [`transition`] for validating legal transition impls
//! - [`validators`] for rebuilding typed machines from persisted data

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_doctests {}

mod syntax;

moddef::moddef!(
    flat (pub) mod {
    },
    flat (pub(crate)) mod {
        machine_ref,
        relation,
        presentation,
        state,
        machine,
        transition,
        validators
    }
);

pub(crate) use presentation::{
    PresentationAttr, PresentationTypesAttr, parse_doc_attrs, parse_present_attrs,
    parse_presentation_types_attr, strip_present_attrs,
};
pub(crate) use syntax::{
    ItemTarget, ModulePath, SourceFingerprint, crate_root_for_file, current_crate_root,
    extract_derives, source_file_fingerprint,
};

use macro_registry::callsite::{
    best_effort_source_context_for_span_or_callsite, module_path_for_span,
};
use proc_macro::TokenStream;
use proc_macro2::Span;
use syn::{Item, ItemImpl, parse_macro_input};

/// Define the legal lifecycle phases for a Statum machine.
///
/// Apply `#[state]` to an enum with unit variants, single-field tuple
/// variants, or named-field variants. Statum generates one marker type per
/// variant plus the state-family traits used by `#[machine]`, `#[transition]`,
/// and `#[validators]`.
#[proc_macro_attribute]
pub fn state(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as Item);
    let input = match input {
        Item::Enum(item_enum) => item_enum,
        other => return invalid_state_target_error(&other).into(),
    };

    // Validate the enum before proceeding
    if let Some(error) = validate_state_enum(&input) {
        return error.into();
    }

    let enum_info = match EnumInfo::from_item_enum(&input) {
        Ok(info) => info,
        Err(err) => return err.to_compile_error().into(),
    };

    // Store metadata in `state_enum_map`
    store_state_enum(&enum_info);

    // Generate structs and implementations dynamically
    let expanded = generate_state_impls(&enum_info);

    TokenStream::from(expanded)
}

/// Define a typed machine that carries durable context across states.
///
/// Apply `#[machine]` to a struct whose first generic parameter is the
/// `#[state]` enum family. Additional type and const generics are supported
/// after that state generic. Statum generates the typed machine surface,
/// builders, the machine-scoped `machine::SomeState` enum, a compatibility
/// alias `machine::State = machine::SomeState`, and helper items such as
/// `machine::Fields` for heterogeneous batch rebuilds.
#[proc_macro_attribute]
pub fn machine(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as Item);
    let input = match input {
        Item::Struct(item_struct) => item_struct,
        other => return invalid_machine_target_error(&other).into(),
    };
    let role = match parse_machine_attr(attr) {
        Ok(role) => role,
        Err(err) => return err.to_compile_error().into(),
    };

    let machine_info = match MachineInfo::from_item_struct(&input, role) {
        Ok(info) => info,
        Err(err) => return err.to_compile_error().into(),
    };

    // Validate the struct before proceeding
    if let Some(error) = validate_machine_struct(&input, &machine_info) {
        return error.into();
    }

    // Store metadata in `machine_map`
    store_machine_struct(&machine_info);

    // Generate any required structs or implementations dynamically
    let expanded = generate_machine_impls(&machine_info, &input);

    TokenStream::from(expanded)
}

/// Declare one nominal opaque type that points at a concrete machine state.
///
/// Apply `#[machine_ref(crate::Machine<crate::State>)]` to a nominal struct or
/// tuple struct when a field or transition parameter should carry an exact
/// machine relation without repeating that relation at every use site.
#[cfg(feature = "machine_ref")]
#[proc_macro_attribute]
pub fn machine_ref(attr: TokenStream, item: TokenStream) -> TokenStream {
    machine_ref::parse_machine_ref(attr, item)
}

#[cfg(not(feature = "machine_ref"))]
#[proc_macro_attribute]
pub fn machine_ref(_attr: TokenStream, _item: TokenStream) -> TokenStream {
    feature_gate_error(
        "machine_ref",
        "`#[machine_ref(...)]` requires the `machine_ref` feature.",
    )
}

/// Validate and generate legal transitions for one source state.
///
/// Apply `#[transition]` to an `impl Machine<CurrentState>` block. Each method
/// must consume `self` and return a legal `Machine<NextState>` shape or a
/// supported wrapper around it, such as `Result<Machine<NextState>, E>`,
/// `Option<Machine<NextState>>`, or
/// `statum::Branch<Machine<Left>, Machine<Right>>`.
#[proc_macro_attribute]
pub fn transition(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as ItemImpl);
    if !attr.is_empty() {
        let message = "Error: `#[transition]` no longer accepts a machine argument.\nFix: write `#[transition]` on an inherent `impl Machine<State>` block and let Statum infer the machine from the impl target.";
        return quote::quote_spanned! { input.impl_token.span =>
            compile_error!(#message);
        }
        .into();
    }
    let module_path = match resolved_current_module_path(input.impl_token.span, "#[transition]") {
        Ok(path) => path,
        Err(err) => return err,
    };

    // -- Step 1: Parse
    let tr_impl = match parse_transition_impl(&input, &module_path) {
        Ok(parsed) => parsed,
        Err(err) => return err.into(),
    };

    if let Some(err) = validate_transition_functions(&tr_impl) {
        return err.into();
    }

    // -- Step 3: Generate new code
    let expanded = generate_transition_impl(&input, &tr_impl);

    // Combine expanded code with the original `impl` if needed
    // or simply return the expanded code
    expanded.into()
}

/// Rebuild typed machines from persisted data.
///
/// Apply `#[validators(Machine)]` to an `impl PersistedRow` block. Statum
/// expects one `is_{state}` method per state variant and generates
/// `into_machine()`, `.into_machines()`, and `.into_machines_by(...)` helpers
/// for typed rehydration. Validator methods can return `Result<T, _>` for
/// ordinary membership checks or `Validation<T>` when rebuild reports should
/// carry stable rejection details through `.build_report()` and
/// `.build_reports()`.
#[cfg(feature = "validators")]
#[proc_macro_attribute]
pub fn validators(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item_impl = parse_macro_input!(item as ItemImpl);
    let span = item_impl.impl_token.span;
    let line_number = best_effort_source_context_for_span_or_callsite(span)
        .line_number
        .max(span.start().line);
    let module_path = match resolved_current_module_path(span, "#[validators]") {
        Ok(path) => path,
        Err(err) => return err,
    };
    parse_validators(attr, item_impl, &module_path, line_number)
}

#[cfg(not(feature = "validators"))]
#[proc_macro_attribute]
pub fn validators(_attr: TokenStream, _item: TokenStream) -> TokenStream {
    feature_gate_error(
        "validators",
        "`#[validators(...)]` requires the `validators` feature.",
    )
}

#[cfg(feature = "validators")]
#[doc(hidden)]
#[proc_macro]
pub fn __statum_emit_validator_methods_impl(input: TokenStream) -> TokenStream {
    validators::emit_validator_methods_impl(input)
}

#[cfg(not(feature = "validators"))]
#[doc(hidden)]
#[proc_macro]
pub fn __statum_emit_validator_methods_impl(_input: TokenStream) -> TokenStream {
    feature_gate_error(
        "validators",
        "`__statum_emit_validator_methods_impl!` requires the `validators` feature.",
    )
}

#[allow(unexpected_cfgs)]
pub(crate) fn is_rust_analyzer() -> bool {
    cfg!(rust_analyzer) || std::env::var("RUST_ANALYZER_INTERNALS").is_ok()
}

#[allow(dead_code)]
fn feature_gate_error(feature: &str, message: &str) -> TokenStream {
    let help = format!("Fix: enable `{feature}` on `statum` or `statum-macros` in Cargo.toml.");

    syn::Error::new(Span::call_site(), format!("{message}\n{help}"))
        .to_compile_error()
        .into()
}

pub(crate) fn resolved_current_module_path(
    span: Span,
    macro_name: &str,
) -> Result<String, TokenStream> {
    let resolved = module_path_for_span(span)
        .or_else(|| best_effort_source_context_for_span_or_callsite(span).module_path)
        .or_else(|| Some("unknown".to_owned()));

    resolved.ok_or_else(|| {
        let message = format!(
            "Internal error: could not resolve the module path for `{macro_name}` at this call site."
        );
        quote::quote_spanned! { span =>
            compile_error!(#message);
        }
        .into()
    })
}
