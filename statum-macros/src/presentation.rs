use quote::ToTokens;
use syn::{Attribute, Expr, ExprLit, Lit, LitStr, Meta, Type};

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

pub fn parse_present_attrs(attrs: &[Attribute]) -> syn::Result<Option<PresentationAttr>> {
    let mut presentation = PresentationAttr::default();
    let mut found = false;

    for attr in attrs.iter().filter(|attr| attr.path().is_ident("present")) {
        found = true;
        attr.parse_nested_meta(|meta| {
            let path = meta.path.clone();
            let Some(ident) = path.get_ident() else {
                return Err(syn::Error::new_spanned(
                    &path,
                    "Error: `#[present(...)]` keys must be simple identifiers like `label = \"...\"`.",
                ));
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
                        "present",
                    )?;
                }
                "description" => {
                    let lit = expect_string_literal(&expr, ident, value_span)?;
                    assign_unique_string_slot(
                        &mut presentation.description,
                        ident,
                        lit.value(),
                        "present",
                    )?;
                }
                "metadata" => {
                    assign_unique_string_slot(
                        &mut presentation.metadata,
                        ident,
                        expr.to_token_stream().to_string(),
                        "present",
                    )?;
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        ident,
                        format!(
                            "Error: unknown `#[present(...)]` key `{}`.\nSupported keys: `label`, `description`, `metadata`.",
                            ident
                        ),
                    ));
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
        attr.parse_nested_meta(|meta| {
            let path = meta.path.clone();
            let Some(ident) = path.get_ident() else {
                return Err(syn::Error::new_spanned(
                    &path,
                    "Error: `#[presentation_types(...)]` keys must be simple identifiers like `state = MyStateMeta`.",
                ));
            };

            let value = meta.value()?;
            let ty: Type = value.parse()?;
            let ty_string = ty.to_token_stream().to_string();

            match ident.to_string().as_str() {
                "machine" => {
                    assign_unique_string_slot(
                        &mut presentation_types.machine,
                        ident,
                        ty_string,
                        "presentation_types",
                    )?;
                }
                "state" => {
                    assign_unique_string_slot(
                        &mut presentation_types.state,
                        ident,
                        ty_string,
                        "presentation_types",
                    )?;
                }
                "transition" => {
                    assign_unique_string_slot(
                        &mut presentation_types.transition,
                        ident,
                        ty_string,
                        "presentation_types",
                    )?;
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        ident,
                        format!(
                            "Error: unknown `#[presentation_types(...)]` key `{}`.\nSupported keys: `machine`, `state`, `transition`.",
                            ident
                        ),
                    ));
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
    attrs.iter()
        .filter(|attr| !attr.path().is_ident("present"))
        .cloned()
        .collect()
}

pub fn parse_doc_attrs(attrs: &[Attribute]) -> syn::Result<Option<String>> {
    let mut lines = Vec::new();

    for attr in attrs.iter().filter(|attr| attr.path().is_ident("doc")) {
        let Meta::NameValue(meta) = &attr.meta else {
            continue;
        };
        let Expr::Lit(ExprLit {
            lit: Lit::Str(lit), ..
        }) = &meta.value
        else {
            continue;
        };
        lines.push(normalize_doc_attr_value(&lit.value()));
    }

    if lines.is_empty() {
        return Ok(None);
    }

    let docs = lines.join("\n");
    if docs.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(docs))
    }
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
        return Err(syn::Error::new(
            span,
            format!(
                "Error: `#[present({ident} = ...)]` expects a string literal.\nFix: write `#[present({ident} = \"...\")]`."
            ),
        ));
    };

    Ok(lit.clone())
}

fn assign_unique_string_slot(
    slot: &mut Option<String>,
    ident: &syn::Ident,
    value: String,
    attr_name: &str,
) -> Result<(), syn::Error> {
    if slot.is_some() {
        let field_label = match attr_name {
            "present" => "presentation field".to_string(),
            _ => format!("{attr_name} field"),
        };
        return Err(syn::Error::new_spanned(
            ident,
            format!(
                "Error: duplicate `#[{attr_name}(...)]` key `{ident}`.\nFix: specify each {field_label} at most once per item.",
            ),
        ));
    }

    *slot = Some(value);
    Ok(())
}

fn parse_optional_type(value: Option<&str>) -> syn::Result<Option<Type>> {
    value
        .map(syn::parse_str::<Type>)
        .transpose()
}

fn normalize_doc_attr_value(value: &str) -> String {
    value
        .split('\n')
        .map(|line| line.strip_prefix(' ').unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::parse_doc_attrs;

    #[test]
    fn parse_doc_attrs_normalizes_and_preserves_blank_lines() {
        let attrs = vec![
            syn::parse_quote!(#[doc = " Summary line"]),
            syn::parse_quote!(#[doc = ""]),
            syn::parse_quote!(#[doc = " Details line"]),
        ];

        assert_eq!(
            parse_doc_attrs(&attrs).expect("docs"),
            Some("Summary line\n\nDetails line".to_owned())
        );
    }

    #[test]
    fn parse_doc_attrs_treats_whitespace_only_as_missing() {
        let attrs = vec![
            syn::parse_quote!(#[doc = " "]),
            syn::parse_quote!(#[doc = "  "]),
        ];

        assert_eq!(parse_doc_attrs(&attrs).expect("docs"), None);
    }
}
