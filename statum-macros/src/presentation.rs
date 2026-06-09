use quote::ToTokens;
use syn::{Attribute, Expr, ExprLit, Lit, LitStr, Type};

use crate::diagnostics::{DiagnosticMessage, compact_display, error_at, error_spanned};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PresentationAttr {
    pub label: Option<String>,
    pub description: Option<String>,
    pub metadata: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PresentationTypesAttr {
    pub machine: Option<String>,
    pub state: Option<String>,
    pub transition: Option<String>,
}

impl PresentationTypesAttr {
    pub fn parse_machine_type(&self) -> syn::Result<Option<Type>> {
        parse_optional_type(self.machine.as_deref())
    }

    pub fn parse_state_type(&self) -> syn::Result<Option<Type>> {
        parse_optional_type(self.state.as_deref())
    }

    pub fn parse_transition_type(&self) -> syn::Result<Option<Type>> {
        parse_optional_type(self.transition.as_deref())
    }
}

pub fn parse_present_attrs_for(
    attrs: &[Attribute],
    owner_context: Option<&str>,
) -> syn::Result<Option<PresentationAttr>> {
    let mut presentation = PresentationAttr::default();
    let mut found = false;

    for attr in attrs.iter().filter(|attr| attr.path().is_ident("present")) {
        found = true;
        if !matches!(attr.meta, syn::Meta::List(_)) {
            let message = DiagnosticMessage::new(format!(
                "`#[present(...)]`{} requires parentheses.",
                owner_suffix(owner_context)
            ))
            .found(format!("`#[{}]`", compact_display(&attr.meta)))
            .expected("`#[present(label = \"...\", description = \"...\")]`")
            .fix(
                "write `#[present(...)]` with key/value pairs inside the parentheses.".to_string(),
            );
            return Err(error_spanned(attr, &message));
        }
        attr.parse_nested_meta(|meta| {
            let path = meta.path.clone();
            let Some(ident) = path.get_ident() else {
                let message = DiagnosticMessage::new(format!(
                    "`#[present(...)]`{} keys must be simple identifiers.",
                    owner_suffix(owner_context)
                ))
                .found(format!("`{}`", compact_display(&path)))
                .expected("`label = \"...\"`, `description = \"...\"`, or `metadata = Expr`")
                .fix("write `#[present(label = \"...\", description = \"...\")]`.");
                return Err(error_spanned(&path, &message));
            };

            let value = meta.value()?;
            let value_span = value.span();
            let expr: Expr = value.parse()?;
            match ident.to_string().as_str() {
                "label" => {
                    let lit = expect_string_literal(&expr, ident, value_span)?;
                    assign_unique_string_slot(
                        &mut presentation.label,
                        ident,
                        lit.value(),
                        format!("\"{}\"", lit.value()),
                        "present",
                        owner_context,
                    )?;
                }
                "description" => {
                    let lit = expect_string_literal(&expr, ident, value_span)?;
                    assign_unique_string_slot(
                        &mut presentation.description,
                        ident,
                        lit.value(),
                        format!("\"{}\"", lit.value()),
                        "present",
                        owner_context,
                    )?;
                }
                "metadata" => {
                    assign_unique_string_slot(
                        &mut presentation.metadata,
                        ident,
                        expr.to_token_stream().to_string(),
                        compact_display(&expr),
                        "present",
                        owner_context,
                    )?;
                }
                _ => {
                    let message = DiagnosticMessage::new(format!(
                        "unknown `#[present(...)]` key `{ident}`{}.",
                        owner_suffix(owner_context),
                    ))
                    .found(format!("`{ident} = {}`", compact_display(&expr)))
                    .expected("`label = \"...\"`, `description = \"...\"`, or `metadata = Expr`")
                    .fix("replace that key or remove it.");
                    return Err(error_spanned(ident, &message));
                }
            }
            Ok(())
        })?;
    }

    if found {
        Ok(Some(presentation))
    } else {
        Ok(None)
    }
}

pub fn parse_presentation_types_attr(
    attrs: &[Attribute],
) -> syn::Result<Option<PresentationTypesAttr>> {
    let mut presentation_types = PresentationTypesAttr::default();
    let mut found = false;

    for attr in attrs
        .iter()
        .filter(|attr| attr.path().is_ident("presentation_types"))
    {
        found = true;
        if !matches!(attr.meta, syn::Meta::List(_)) {
            let message = DiagnosticMessage::new(
                "`#[presentation_types(...)]` requires parentheses.",
            )
            .found(format!("`#[{}]`", compact_display(&attr.meta)))
            .expected(
                "`#[presentation_types(machine = MachineMeta, state = StateMeta, transition = TransitionMeta)]`",
            )
            .fix("write `#[presentation_types(...)]` with key/type pairs inside the parentheses.".to_string());
            return Err(error_spanned(attr, &message));
        }
        attr.parse_nested_meta(|meta| {
            let path = meta.path.clone();
            let Some(ident) = path.get_ident() else {
                let message = DiagnosticMessage::new(
                    "`#[presentation_types(...)]` keys must be simple identifiers.",
                )
                .found(format!("`{}`", compact_display(&path)))
                .expected("`machine = MachineMeta`, `state = StateMeta`, or `transition = TransitionMeta`")
                .fix("write `#[presentation_types(state = StateMeta)]` with plain identifier keys.");
                return Err(error_spanned(&path, &message));
            };

            let value = meta.value()?;
            let ty: Type = value.parse()?;
            let ty_string = ty.to_token_stream().to_string();

            match ident.to_string().as_str() {
                "machine" => {
                    assign_unique_string_slot(
                        &mut presentation_types.machine,
                        ident,
                        ty_string.clone(),
                        ty_string,
                        "presentation_types",
                        None,
                    )?;
                }
                "state" => {
                    assign_unique_string_slot(
                        &mut presentation_types.state,
                        ident,
                        ty_string.clone(),
                        ty_string,
                        "presentation_types",
                        None,
                    )?;
                }
                "transition" => {
                    assign_unique_string_slot(
                        &mut presentation_types.transition,
                        ident,
                        ty_string.clone(),
                        ty_string,
                        "presentation_types",
                        None,
                    )?;
                }
                _ => {
                    let message = DiagnosticMessage::new(format!(
                        "unknown `#[presentation_types(...)]` key `{ident}`."
                    ))
                    .found(format!("`{ident} = {ty_string}`"))
                    .expected(
                        "`machine = MachineMeta`, `state = StateMeta`, or `transition = TransitionMeta`",
                    )
                    .fix("replace that key or remove it.");
                    return Err(error_spanned(ident, &message));
                }
            }

            Ok(())
        })?;
    }

    if found {
        Ok(Some(presentation_types))
    } else {
        Ok(None)
    }
}

pub fn strip_present_attrs(attrs: &[Attribute]) -> Vec<Attribute> {
    attrs
        .iter()
        .filter(|attr| !attr.path().is_ident("present"))
        .cloned()
        .collect()
}

fn owner_suffix(owner_context: Option<&str>) -> String {
    owner_context
        .map(|context| format!(" on {context}"))
        .unwrap_or_default()
}

fn expect_string_literal(
    expr: &Expr,
    ident: &syn::Ident,
    span: proc_macro2::Span,
) -> Result<LitStr, syn::Error> {
    let Expr::Lit(ExprLit {
        lit: Lit::Str(lit), ..
    }) = expr
    else {
        let message = DiagnosticMessage::new(format!(
            "`#[present({ident} = ...)]` expects a string literal."
        ))
        .found(format!("`{ident} = {}`", compact_display(expr)))
        .expected(format!("`{ident} = \"...\"`"))
        .fix(format!("write `#[present({ident} = \"...\")]`."));
        return Err(error_at(span, &message));
    };

    Ok(lit.clone())
}

fn assign_unique_string_slot(
    slot: &mut Option<String>,
    ident: &syn::Ident,
    stored_value: String,
    found_display: String,
    attr_name: &str,
    owner_context: Option<&str>,
) -> Result<(), syn::Error> {
    if slot.is_some() {
        let field_label = match attr_name {
            "present" => "presentation field".to_string(),
            _ => format!("{attr_name} field"),
        };
        let message = DiagnosticMessage::new(format!(
            "duplicate `#[{attr_name}(...)]` key `{ident}`{}.",
            owner_suffix(owner_context),
        ))
        .found(format!("`{ident} = {found_display}`"))
        .expected(format!("one `{ident}` {field_label} entry"))
        .fix(format!(
            "specify `{ident}` at most once inside `#[{attr_name}(...)]`."
        ));
        return Err(error_spanned(ident, &message));
    }

    *slot = Some(stored_value);
    Ok(())
}

fn parse_optional_type(value: Option<&str>) -> syn::Result<Option<Type>> {
    value.map(syn::parse_str::<Type>).transpose()
}
