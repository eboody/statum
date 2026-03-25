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
        presentation,
        state,
        machine,
        transition,
        validators
    }
);

pub(crate) use presentation::{
    PresentationAttr, PresentationTypesAttr, parse_present_attrs, parse_presentation_types_attr,
    strip_present_attrs,
};
pub(crate) use syntax::{
    ItemTarget, ModulePath, SourceFingerprint, crate_root_for_file, current_crate_root,
    extract_derives, source_file_fingerprint,
};

use macro_registry::callsite::{current_module_path_at_line, current_module_path_opt};
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
pub fn machine(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as Item);
    let input = match input {
        Item::Struct(item_struct) => item_struct,
        other => return invalid_machine_target_error(&other).into(),
    };

    let machine_info = match MachineInfo::from_item_struct(&input) {
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

/// Validate and generate legal transitions for one source state.
///
/// Apply `#[transition]` to an `impl Machine<CurrentState>` block. Each method
/// must consume `self` and return a legal `Machine<NextState>` shape or a
/// supported wrapper around it, such as `Result<Machine<NextState>, E>`,
/// `Option<Machine<NextState>>`, or
/// `statum::Branch<Machine<Left>, Machine<Right>>`.
#[proc_macro_attribute]
pub fn transition(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as ItemImpl);

    // -- Step 1: Parse
    let tr_impl = match parse_transition_impl(&input) {
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
#[proc_macro_attribute]
pub fn validators(attr: TokenStream, item: TokenStream) -> TokenStream {
    let module_path = match resolved_current_module_path(Span::call_site(), "#[validators]") {
        Ok(path) => path,
        Err(err) => return err,
    };
    parse_validators(attr, item, &module_path)
}

fn resolved_current_module_path(span: Span, macro_name: &str) -> Result<String, TokenStream> {
    let line_number = span.start().line;
    let resolved = if line_number == 0 {
        current_module_path_opt()
    } else {
        current_module_path_at_line(line_number).or_else(current_module_path_opt)
    };

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
