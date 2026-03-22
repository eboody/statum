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

mod syntax;

moddef::moddef!(
    flat (pub) mod {
    },
    flat (pub(crate)) mod {
        state,
        machine,
        transition,
        validators
    }
);

pub(crate) use syntax::{ItemTarget, ModulePath, extract_derives};

use crate::{MachinePath, ensure_machine_loaded_by_name, unique_loaded_machine_elsewhere};
use macro_registry::callsite::current_module_path_opt;
use proc_macro::TokenStream;
use proc_macro2::Span;
use syn::{Item, ItemImpl, parse_macro_input};

/// Define the legal lifecycle phases for a Statum machine.
///
/// Apply `#[state]` to an enum with unit variants and single-field tuple
/// variants. Statum generates one marker type per variant plus the state-family
/// traits used by `#[machine]`, `#[transition]`, and `#[validators]`.
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
    let expanded = generate_state_impls(&enum_info.module_path);

    TokenStream::from(expanded)
}

/// Define a typed machine that carries durable context across states.
///
/// Apply `#[machine]` to a struct whose first generic parameter is the
/// `#[state]` enum family. Statum generates the typed machine surface, builders,
/// the machine-scoped `machine::SomeState` enum, a compatibility alias
/// `machine::State = machine::SomeState`, and helper items such as
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
/// supported wrapper around it, such as `Result<Machine<NextState>, E>`.
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

    let module_path = match resolved_current_module_path(tr_impl.machine_span, "#[transition]") {
        Ok(path) => path,
        Err(err) => return err,
    };

    let machine_path: MachinePath = module_path.clone().into();
    // `include!` gives the transition macro the included file as its source context,
    // so exact module lookup can miss the already-loaded parent machine.
    let machine_info_owned = ensure_machine_loaded_by_name(&machine_path, &tr_impl.machine_name)
        .or_else(|| unique_loaded_machine_elsewhere(&tr_impl.machine_name));
    let machine_info = match machine_info_owned.as_ref() {
        Some(info) => info,
        None => {
            return missing_transition_machine_error(
                &tr_impl.machine_name,
                &module_path,
                tr_impl.machine_span,
            )
            .into();
        }
    };

    if let Some(err) = validate_transition_functions(&tr_impl, machine_info) {
        return err.into();
    }

    // -- Step 3: Generate new code
    let expanded = generate_transition_impl(&input, &tr_impl, machine_info);

    // Combine expanded code with the original `impl` if needed
    // or simply return the expanded code
    expanded.into()
}

/// Rebuild typed machines from persisted data.
///
/// Apply `#[validators(Machine)]` to an `impl PersistedRow` block. Statum
/// expects one `is_{state}` method per state variant and generates
/// `into_machine()`, `.into_machines()`, and `.into_machines_by(...)` helpers
/// for typed rehydration.
#[proc_macro_attribute]
pub fn validators(attr: TokenStream, item: TokenStream) -> TokenStream {
    let module_path = match resolved_current_module_path(Span::call_site(), "#[validators]") {
        Ok(path) => path,
        Err(err) => return err,
    };
    parse_validators(attr, item, &module_path)
}

fn resolved_current_module_path(span: Span, macro_name: &str) -> Result<String, TokenStream> {
    current_module_path_opt().ok_or_else(|| {
        let message = format!(
            "Internal error: could not resolve the module path for `{macro_name}` at this call site."
        );
        quote::quote_spanned! { span =>
            compile_error!(#message);
        }
        .into()
    })
}
