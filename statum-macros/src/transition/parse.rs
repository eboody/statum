use super::resolve::extract_impl_machine_and_state;
use crate::diagnostics::{DiagnosticMessage, compact_display, compile_error_at};
use crate::{PresentationAttr, parse_present_attrs, strip_present_attrs};
use proc_macro2::{Span, TokenStream};
use syn::meta::ParseNestedMeta;
use syn::spanned::Spanned;
use syn::{FnArg, Ident, ImplItem, ImplItemFn, ItemImpl, ReturnType, Type};

#[allow(unused)]
pub struct TransitionFn {
    pub name: Ident,
    pub attrs: Vec<syn::Attribute>,
    pub presentation: Option<PresentationAttr>,
    pub introspection: Option<TransitionIntrospectAttr>,
    pub has_receiver: bool,
    pub return_type: Option<Type>,
    pub return_type_span: Option<Span>,
    pub machine_name: String,
    pub source_state: String,
    pub span: proc_macro2::Span,
}

#[derive(Clone)]
pub struct TransitionIntrospectAttr {
    pub return_type: Type,
    pub span: Span,
}

impl TransitionFn {
}

pub struct TransitionImpl {
    pub target_type: Type,
    pub machine_name: String,
    pub machine_span: Span,
    pub source_state: String,
    pub source_state_span: Span,
    pub attrs: Vec<syn::Attribute>,
    pub functions: Vec<TransitionFn>,
}

pub fn parse_transition_impl(item_impl: &ItemImpl) -> Result<TransitionImpl, TokenStream> {
    let target_type = *item_impl.self_ty.clone();
    let Some((machine_name, machine_span, source_state, source_state_span)) =
        extract_impl_machine_and_state(&target_type)
    else {
        let message = DiagnosticMessage::new(
            "`#[transition]` must be applied to an impl target like `Machine<State>`.",
        )
        .found(format!("`impl {}`", compact_display(&target_type)))
        .expected("`#[transition] impl WorkflowMachine<Draft> { ... }`")
        .fix("apply `#[transition]` to an impl for the local `#[machine]` type and one concrete state marker.");
        return Err(compile_error_at(target_type.span(), &message));
    };

    let mut functions = Vec::new();
    for item in &item_impl.items {
        if let ImplItem::Fn(method) = item {
            functions.push(parse_transition_fn(method, &machine_name, &source_state)?);
        }
    }

    Ok(TransitionImpl {
        target_type,
        machine_name,
        machine_span,
        source_state,
        source_state_span,
        attrs: item_impl.attrs.clone(),
        functions,
    })
}

fn parse_transition_fn(
    method: &ImplItemFn,
    machine_name: &str,
    source_state: &str,
) -> Result<TransitionFn, TokenStream> {
    let has_receiver = matches!(method.sig.inputs.first(), Some(FnArg::Receiver(_)));

    let return_type = match &method.sig.output {
        ReturnType::Type(_, ty) => Some(*ty.clone()),
        ReturnType::Default => None,
    };
    let return_type_span = match &method.sig.output {
        ReturnType::Type(_, ty) => Some(ty.span()),
        ReturnType::Default => None,
    };

    Ok(TransitionFn {
        name: method.sig.ident.clone(),
        attrs: method.attrs.clone(),
        presentation: parse_present_attrs(&method.attrs).map_err(|err| err.to_compile_error())?,
        introspection: parse_transition_introspect_attrs(&method.attrs)
            .map_err(|err| err.to_compile_error())?,
        has_receiver,
        return_type,
        return_type_span,
        machine_name: machine_name.to_owned(),
        source_state: source_state.to_owned(),
        span: method.span(),
    })
}

pub(super) fn strip_present_attrs_from_transition_impl(input: &ItemImpl) -> ItemImpl {
    let mut sanitized = input.clone();
    sanitized.attrs = strip_present_attrs(&sanitized.attrs);
    for item in &mut sanitized.items {
        if let ImplItem::Fn(method) = item {
            method.attrs = strip_present_attrs(&method.attrs);
            method.attrs = strip_transition_introspect_attrs(&method.attrs);
        }
    }
    sanitized
}

fn parse_transition_introspect_attrs(
    attrs: &[syn::Attribute],
) -> syn::Result<Option<TransitionIntrospectAttr>> {
    let mut return_type = None;
    let mut found = false;
    let mut attr_span = None;

    for attr in attrs.iter().filter(|attr| attr.path().is_ident("introspect")) {
        found = true;
        attr_span = Some(attr.span());
        if !matches!(attr.meta, syn::Meta::List(_)) {
            let message = DiagnosticMessage::new("`#[introspect(...)]` requires parentheses.")
                .found(format!("`#[{}]`", compact_display(&attr.meta)))
                .expected("`#[introspect(return = WorkflowMachine<NextState>)]`")
                .fix("write `#[introspect(return = ...)]` on the transition method.".to_string());
            return Err(syn::Error::new(attr.span(), message.render()));
        }
        attr.parse_nested_meta(|meta| parse_transition_introspect_meta(meta, &mut return_type))?;
    }

    if !found {
        return Ok(None);
    }

    let Some(return_type) = return_type else {
        return Err(syn::Error::new(
            attr_span.unwrap_or(Span::call_site()),
            DiagnosticMessage::new("`#[introspect(...)]` requires `return = <Type>`.")
                .expected("`#[introspect(return = WorkflowMachine<NextState>)]`")
                .fix("declare the exact transition return shape with `return = ...`.".to_string())
                .render(),
        ));
    };

    Ok(Some(TransitionIntrospectAttr {
        return_type,
        span: attr_span.unwrap_or(Span::call_site()),
    }))
}

fn parse_transition_introspect_meta(
    meta: ParseNestedMeta<'_>,
    return_type: &mut Option<Type>,
) -> syn::Result<()> {
    let path = meta.path.clone();
    let Some(ident) = path.get_ident() else {
        let message = DiagnosticMessage::new(
            "`#[introspect(...)]` keys must be simple identifiers.",
        )
        .found(format!("`{}`", compact_display(&path)))
        .expected("`return = WorkflowMachine<NextState>`")
        .fix("write `#[introspect(return = ...)]`.".to_string());
        return Err(syn::Error::new_spanned(&path, message.render()));
    };

    match ident.to_string().as_str() {
        "return" => {
            if return_type.is_some() {
                let message = DiagnosticMessage::new(
                    "duplicate `#[introspect(...)]` key `return`.",
                )
                .found("`return = ...`")
                .expected("one `return = ...` entry")
                .fix("specify `return = ...` at most once per method.");
                return Err(syn::Error::new_spanned(ident, message.render()));
            }

            let value = meta.value()?;
            *return_type = Some(value.parse()?);
            Ok(())
        }
        _ => {
            let message = DiagnosticMessage::new(format!(
                "unknown `#[introspect(...)]` key `{ident}`."
            ))
            .found(format!("`{ident} = ...`"))
            .expected("`return = WorkflowMachine<NextState>`")
            .fix("use the `return` key or remove the extra entry.".to_string());
            Err(syn::Error::new_spanned(ident, message.render()))
        }
    }
}

pub(super) fn strip_transition_introspect_attrs(
    attrs: &[syn::Attribute],
) -> Vec<syn::Attribute> {
    attrs.iter()
        .filter(|attr| !attr.path().is_ident("introspect"))
        .cloned()
        .collect()
}
