use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote_spanned};
use syn::spanned::Spanned;
use syn::{Item, LitStr};

#[derive(Clone, Debug)]
pub(crate) struct DiagnosticMessage {
    summary: String,
    sections: Vec<(&'static str, String)>,
}

impl DiagnosticMessage {
    pub(crate) fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            sections: Vec::new(),
        }
    }

    pub(crate) fn found(self, value: impl Into<String>) -> Self {
        self.section("Found", value)
    }

    pub(crate) fn expected(self, value: impl Into<String>) -> Self {
        self.section("Expected", value)
    }

    pub(crate) fn fix(self, value: impl Into<String>) -> Self {
        self.section("Fix", value)
    }

    pub(crate) fn reason(self, value: impl Into<String>) -> Self {
        self.section("Reason", value)
    }

    pub(crate) fn note(self, value: impl Into<String>) -> Self {
        self.section("Note", value)
    }

    pub(crate) fn candidates(self, value: impl Into<String>) -> Self {
        self.section("Candidates", value)
    }

    pub(crate) fn assumption(self, value: impl Into<String>) -> Self {
        self.section("Assumption", value)
    }

    pub(crate) fn help(self, value: impl Into<String>) -> Self {
        self.section("Help", value)
    }

    pub(crate) fn section(mut self, label: &'static str, value: impl Into<String>) -> Self {
        let value = value.into();
        if !value.trim().is_empty() {
            self.sections.push((label, value));
        }
        self
    }

    pub(crate) fn render(&self) -> String {
        let mut rendered = format!("Error: {}", self.summary);
        for (label, value) in &self.sections {
            rendered.push('\n');
            rendered.push_str(&render_section(label, value));
        }
        rendered
    }
}

pub(crate) fn compile_error_at(span: Span, message: &DiagnosticMessage) -> TokenStream {
    let rendered = LitStr::new(&message.render(), span);
    quote_spanned! { span =>
        compile_error!(#rendered);
    }
}

pub(crate) fn error_at(span: Span, message: &DiagnosticMessage) -> syn::Error {
    syn::Error::new(span, message.render())
}

pub(crate) fn error_spanned<T>(node: &T, message: &DiagnosticMessage) -> syn::Error
where
    T: Spanned + ToTokens,
{
    syn::Error::new_spanned(node, message.render())
}

pub(crate) fn compact_display<T>(value: &T) -> String
where
    T: ToTokens + ?Sized,
{
    normalize_tokens(&value.to_token_stream().to_string())
}

pub(crate) fn compact_text(value: &str) -> String {
    normalize_tokens(value)
}

pub(crate) fn item_signature(item: &Item) -> String {
    match item {
        Item::Const(item) => format!("`const {}: ... = ...;`", item.ident),
        Item::Enum(item) => format!(
            "`enum {}{} {{ ... }}`",
            item.ident,
            generics_suffix(&compact_display(&item.generics))
        ),
        Item::ExternCrate(item) => format!("`extern crate {};`", item.ident),
        Item::Fn(item) => format!("`{} {{ ... }}`", compact_display(&item.sig)),
        Item::ForeignMod(_) => "`extern \"...\" { ... }`".to_string(),
        Item::Impl(item) => {
            let trait_prefix = item
                .trait_
                .as_ref()
                .map(|(_, path, _)| format!("{} for ", compact_display(path)))
                .unwrap_or_default();
            format!(
                "`impl {}{} {{ ... }}`",
                trait_prefix,
                compact_display(&item.self_ty)
            )
        }
        Item::Macro(item) => format!("`{}! {{ ... }}`", compact_display(&item.mac.path)),
        Item::Mod(item) => format!("`mod {} {{ ... }}`", item.ident),
        Item::Static(item) => format!("`static {}: ... = ...;`", item.ident),
        Item::Struct(item) => format!(
            "`struct {}{} {{ ... }}`",
            item.ident,
            generics_suffix(&compact_display(&item.generics))
        ),
        Item::Trait(item) => format!(
            "`trait {}{} {{ ... }}`",
            item.ident,
            generics_suffix(&compact_display(&item.generics))
        ),
        Item::TraitAlias(item) => format!(
            "`trait {}{} = ...;`",
            item.ident,
            generics_suffix(&compact_display(&item.generics))
        ),
        Item::Type(item) => format!(
            "`type {}{} = ...;`",
            item.ident,
            generics_suffix(&compact_display(&item.generics))
        ),
        Item::Union(item) => format!(
            "`union {}{} {{ ... }}`",
            item.ident,
            generics_suffix(&compact_display(&item.generics))
        ),
        Item::Use(item) => format!("`use {};`", compact_display(&item.tree)),
        _ => format!("`{}`", compact_display(item)),
    }
}

fn render_section(label: &str, value: &str) -> String {
    let indent = " ".repeat(label.len() + 2);
    let mut lines = value.lines();
    let Some(first) = lines.next() else {
        return format!("{label}:");
    };

    let mut rendered = format!("{label}: {first}");
    for line in lines {
        rendered.push('\n');
        rendered.push_str(&indent);
        rendered.push_str(line);
    }
    rendered
}

fn normalize_tokens(tokens: &str) -> String {
    let mut normalized = tokens.to_owned();
    for (from, to) in [
        (" :: ", "::"),
        (":: ", "::"),
        (" ::", "::"),
        (" < ", "<"),
        ("< ", "<"),
        (" >", ">"),
        (" ( ", "("),
        ("( ", "("),
        (" )", ")"),
        (" [ ", "["),
        ("[ ", "["),
        (" ]", "]"),
        (" { ", " { "),
        (" ;", ";"),
        (" , ", ", "),
        (" : ", ": "),
        ("< '", "<'"),
        ("& '", "&'"),
        (" & ", " &"),
    ] {
        normalized = normalized.replace(from, to);
    }

    while normalized.contains("> >") {
        normalized = normalized.replace("> >", ">>");
    }
    while normalized.contains("  ") {
        normalized = normalized.replace("  ", " ");
    }

    normalized.trim().to_string()
}

fn generics_suffix(generics: &str) -> String {
    if generics.is_empty() || generics == "<>" {
        String::new()
    } else {
        generics.to_string()
    }
}
