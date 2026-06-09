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

mod contracts;
mod diagnostics;
mod machine;
mod presentation;
mod source;
mod state;
mod transition;
mod validators;

pub(crate) use machine::{
    LoadedMachineLookupFailure, MachineInfo, MachinePath, expand_machine,
    format_loaded_machine_candidates, invalid_machine_target_error,
    lookup_loaded_machine_in_module, lookup_unique_loaded_machine_by_name,
};
pub(crate) use state::{
    EnumInfo, LoadedStateLookupFailure, StateModulePath, VariantInfo, VariantShape, expand_state,
    format_loaded_state_candidates, invalid_state_target_error, lookup_loaded_state_enum,
    lookup_loaded_state_enum_by_name, to_snake_case,
};
pub(crate) use transition::expand_transition;
pub(crate) use validators::parse_validators;

pub(crate) use presentation::{
    PresentationAttr, PresentationTypesAttr, parse_present_attrs_for,
    parse_presentation_types_attr, strip_present_attrs,
};
pub(crate) use source::{
    ItemTarget, ModulePath, SourceFingerprint, crate_root_for_file, current_crate_root,
    extract_derives, source_file_fingerprint,
};

use crate::diagnostics::DiagnosticMessage;
use crate::source::{current_module_path_opt, module_path_for_span, source_info_for_span};
use proc_macro::TokenStream;
use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::{Item, ItemImpl, parse_macro_input};

pub(crate) fn strict_introspection_enabled() -> bool {
    cfg!(feature = "strict-introspection")
}

/// Define the legal lifecycle phases for a Statum machine.
///
/// Apply `#[state]` to an enum with unit variants, single-field tuple
/// variants, or named-field variants. Statum generates one marker type per
/// variant plus the state-family traits used by `#[machine]`, `#[transition]`,
/// and `#[validators]`.
#[proc_macro_attribute]
pub fn state(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return syn::Error::new(
            Span::call_site(),
            DiagnosticMessage::new("`#[state]` does not accept arguments.")
                .found(format!("`#[state({attr})]`"))
                .expected("`#[state] enum WorkflowState { Draft, Review(ReviewData) }`")
                .fix("remove the attribute arguments and describe states with enum variants instead.".to_string())
                .render(),
        )
        .to_compile_error()
        .into();
    }
    let input = parse_macro_input!(item as Item);
    let input = match input {
        Item::Enum(item_enum) => item_enum,
        other => return invalid_state_target_error(&other).into(),
    };
    expand_state(input).into()
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
    if !attr.is_empty() {
        return syn::Error::new(
            Span::call_site(),
            DiagnosticMessage::new("`#[machine]` does not accept arguments.")
                .found(format!("`#[machine({attr})]`"))
                .expected("`#[machine] struct WorkflowMachine<WorkflowState> { ... }`")
                .fix("remove the attribute arguments and link the machine to `#[state]` through its first generic parameter.".to_string())
                .render(),
        )
        .to_compile_error()
        .into();
    }
    let input = parse_macro_input!(item as Item);
    let input = match input {
        Item::Struct(item_struct) => item_struct,
        other => return invalid_machine_target_error(&other).into(),
    };
    expand_machine(input).into()
}

/// Validate and generate legal transitions for one source state.
///
/// Apply `#[transition]` to an `impl Machine<CurrentState>` block. Each method
/// must consume `self` and return a legal `Machine<NextState>` shape or a
/// source-declared type alias that expands to that shape, or a supported
/// wrapper around it, such as `Result<Machine<NextState>, E>`,
/// `Option<Machine<NextState>>`, or
/// `statum::Branch<Machine<Left>, Machine<Right>>`.
///
/// When the `strict-introspection` feature is enabled, transition graph
/// semantics must be directly readable from the written return type or from a
/// local `#[introspect(return = ...)]` escape hatch on the method.
#[proc_macro_attribute]
pub fn transition(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    if !attr.is_empty() {
        return syn::Error::new(
            Span::call_site(),
            DiagnosticMessage::new("`#[transition]` does not accept arguments.")
                .found(format!("`#[transition({attr})]`"))
                .expected("`#[transition] impl WorkflowMachine<Draft> { ... }`")
                .fix("remove the attribute arguments and declare transition behavior with methods inside the impl block.".to_string())
                .render(),
        )
        .to_compile_error()
        .into();
    }
    let input = parse_macro_input!(item as ItemImpl);
    expand_transition(input).into()
}

/// Rebuild typed machines from persisted data.
///
/// Apply `#[validators(Machine)]` or an anchored path such as
/// `#[validators(self::path::Machine)]`,
/// `#[validators(super::path::Machine)]`, or
/// `#[validators(crate::path::Machine)]` to an `impl PersistedRow` block.
/// Statum expects one `is_{state}` method per state variant and generates
/// `into_machine()`, `.into_machines()`, and `.into_machines_by(...)` helpers
/// for typed rehydration. Validator methods can return `Result<T, _>` for
/// ordinary membership checks or `Validation<T>` when rebuild reports should
/// carry stable rejection details through `.build_report()` and
/// `.build_reports()`. In relaxed mode, bare multi-segment paths like
/// `#[validators(flow::Machine)]` are treated as local child-module paths, not
/// imported aliases or re-exports. If Statum cannot resolve that local path,
/// it emits a compile error asking for an anchored path instead.
#[proc_macro_attribute]
pub fn validators(attr: TokenStream, item: TokenStream) -> TokenStream {
    if attr.is_empty() {
        return syn::Error::new(
            Span::call_site(),
            DiagnosticMessage::new("`#[validators(...)]` requires a machine path.")
                .expected("`#[validators(WorkflowMachine)] impl PersistedRow { ... }`")
                .fix("pass the target Statum machine type in the attribute, for example `#[validators(self::flow::WorkflowMachine)]`.".to_string())
                .render(),
        )
        .to_compile_error()
        .into();
    }
    let item_impl = parse_macro_input!(item as ItemImpl);
    let module_path = match resolved_current_module_path(item_impl.self_ty.span(), "#[validators]")
    {
        Ok(path) => path,
        Err(err) => return err,
    };
    parse_validators(attr, item_impl, &module_path)
}

pub(crate) fn resolved_current_module_path(
    span: Span,
    macro_name: &str,
) -> Result<String, TokenStream> {
    let resolved = module_path_for_span(span)
        .or_else(current_module_path_opt)
        .or_else(|| {
            source_info_for_span(span)
                .is_none()
                .then_some("crate".to_string())
        });

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
