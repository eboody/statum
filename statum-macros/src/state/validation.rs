use proc_macro2::TokenStream;
use syn::{Fields, Item, ItemEnum};

use crate::diagnostics::{DiagnosticMessage, compact_display, item_signature};
use crate::source::ItemTarget;

pub fn invalid_state_target_error(item: &Item) -> TokenStream {
    let target = ItemTarget::from(item);
    let expected_name = target.name().unwrap_or("WorkflowState");
    let message = DiagnosticMessage::new("#[state] must be applied to an enum.")
        .found(item_signature(item))
        .expected(format!(
            "`enum {expected_name} {{ Draft, InReview(InReviewData) }}`"
        ))
        .fix(match target.name() {
            Some(name) => format!(
                "change `{name}` from {} {} into a `#[state]` enum, or remove `#[state]`.",
                target.article(),
                target.kind()
            ),
            None => "apply `#[state]` to an enum item instead.".to_string(),
        });
    syn::Error::new(target.span(), message.render()).to_compile_error()
}

pub fn validate_state_enum(item: &ItemEnum) -> Option<TokenStream> {
    validate_state_enum_shape(item)
        .err()
        .map(|err| err.to_compile_error())
}

pub(super) fn validate_state_enum_shape(item: &ItemEnum) -> syn::Result<()> {
    let enum_name = item.ident.to_string();

    if !item.generics.params.is_empty() {
        let generics_display = compact_display(&item.generics);
        return Err(syn::Error::new_spanned(
            &item.generics,
            DiagnosticMessage::new(format!(
                "`#[state]` enum `{enum_name}` cannot declare generics."
            ))
            .found(format!("`enum {enum_name}{generics_display} {{ ... }}`"))
            .expected(format!("`enum {enum_name} {{ Draft, Review(ReviewData) }}`"))
            .fix(format!(
                "keep `{enum_name}` non-generic and move the generic data into a payload type such as `ReviewData<T>`."
            ))
            .render(),
        ));
    }

    if item.variants.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.ident,
            DiagnosticMessage::new(format!(
                "`#[state]` enum `{enum_name}` must declare at least one variant."
            ))
            .found(format!("`enum {enum_name} {{}}`"))
            .expected(format!(
                "`enum {enum_name} {{ Draft, InReview(InReviewData) }}`"
            ))
            .fix("add at least one unit state or single-payload state variant.")
            .render(),
        ));
    }

    for variant in &item.variants {
        if let Some(attr_name) = cfg_like_attr_name(&variant.attrs) {
            let variant_name = variant.ident.to_string();
            return Err(syn::Error::new_spanned(
                variant,
                DiagnosticMessage::new(format!(
                    "`#[state]` enum `{enum_name}` variant `{variant_name}` uses `#[{attr_name}]`, but Statum does not support conditionally compiled state variants."
                ))
                .found(format!("`{}`", compact_display(variant)))
                .expected(format!("an unconditional `{variant_name}` variant inside `{enum_name}`"))
                .fix("move the cfg gate to the whole `#[state]` enum or split cfg-specific workflows into separate modules.")
                .render(),
            ));
        }

        for field in variant.fields.iter() {
            if let Some(attr_name) = cfg_like_attr_name(&field.attrs) {
                let variant_name = variant.ident.to_string();
                let field_name = field
                    .ident
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "payload field".to_string());
                return Err(syn::Error::new_spanned(
                    field,
                    DiagnosticMessage::new(format!(
                        "`#[state]` enum `{enum_name}` variant `{variant_name}` field `{field_name}` uses `#[{attr_name}]`, but Statum does not support conditionally compiled state payload fields."
                    ))
                    .found(format!("`{}`", compact_display(field)))
                    .expected(format!(
                        "an unconditional payload field for `{variant_name}`"
                    ))
                    .fix("move the cfg gate to the whole variant or wrap the cfg-specific payload shape behind a separate type.")
                    .render(),
                ));
            }
        }

        match &variant.fields {
            Fields::Unit => {}
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {}
            Fields::Unnamed(fields) => {
                let variant_name = variant.ident.to_string();
                let field_count = fields.unnamed.len();
                return Err(syn::Error::new_spanned(
                    variant,
                    DiagnosticMessage::new(format!(
                        "`#[state]` enum `{enum_name}` variant `{variant_name}` carries {field_count} fields, but Statum supports at most one payload type per state."
                    ))
                    .found(format!("`{}`", compact_display(variant)))
                    .expected(format!("`{variant_name}({variant_name}Data)`"))
                    .fix(format!(
                        "wrap the current fields in a payload type like `struct {variant_name}Data {{ ... }}` and use `enum {enum_name} {{ {variant_name}({variant_name}Data) }}`."
                    ))
                    .render(),
                ));
            }
            Fields::Named(fields) if fields.named.is_empty() => {
                let variant_name = variant.ident.to_string();
                return Err(syn::Error::new_spanned(
                    variant,
                    DiagnosticMessage::new(format!(
                        "`#[state]` enum `{enum_name}` variant `{variant_name}` uses empty named fields."
                    ))
                    .found(format!("`{}`", compact_display(variant)))
                    .expected(format!("`{variant_name}` or `{variant_name} {{ field: Type }}`"))
                    .fix(format!(
                        "use `{variant_name}` for a unit state or add at least one named payload field."
                    ))
                    .render(),
                ));
            }
            Fields::Named(_) => {}
        }
    }

    Ok(())
}

fn cfg_like_attr_name(attrs: &[syn::Attribute]) -> Option<&'static str> {
    attrs.iter().find_map(|attr| {
        if attr.path().is_ident("cfg") {
            Some("cfg")
        } else if attr.path().is_ident("cfg_attr") {
            Some("cfg_attr")
        } else {
            None
        }
    })
}
