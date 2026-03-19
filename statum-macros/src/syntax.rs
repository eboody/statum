use core::fmt;

use macro_registry::registry;
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::spanned::Spanned;
use syn::{Attribute, Item, Path};

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct ModulePath(pub String);

impl AsRef<str> for ModulePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModulePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl registry::RegistryKey for ModulePath {
    fn from_module_path(module_path: String) -> Self {
        Self(module_path)
    }
}

impl ToTokens for ModulePath {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match syn::parse_str::<syn::Path>(&self.0) {
            Ok(path) => path.to_tokens(tokens),
            Err(_) => {
                let message = syn::LitStr::new(
                    &format!("Invalid module path tokenization for `{self}`."),
                    Span::call_site(),
                );
                tokens.extend(quote! { compile_error!(#message); });
            }
        }
    }
}

impl From<&str> for ModulePath {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for ModulePath {
    fn from(value: String) -> Self {
        Self(value)
    }
}

pub(crate) fn extract_derives(attr: &Attribute) -> Option<Vec<String>> {
    if !attr.path().is_ident("derive") {
        return None;
    }

    attr.meta
        .require_list()
        .ok()?
        .parse_args_with(syn::punctuated::Punctuated::<Path, syn::Token![,]>::parse_terminated)
        .ok()
        .map(|paths| {
            paths
                .iter()
                .map(|path| path.to_token_stream().to_string())
                .collect()
        })
}

pub(crate) struct ItemTarget {
    kind: &'static str,
    name: Option<String>,
    span: Span,
}

impl ItemTarget {
    pub(crate) fn article(&self) -> &'static str {
        match self.kind.chars().next() {
            Some('a' | 'e' | 'i' | 'o' | 'u') => "an",
            _ => "a",
        }
    }

    pub(crate) fn kind(&self) -> &'static str {
        self.kind
    }

    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub(crate) fn span(&self) -> Span {
        self.span
    }
}

impl From<&Item> for ItemTarget {
    fn from(item: &Item) -> Self {
        match item {
            Item::Const(item) => Self {
                kind: "const item",
                name: Some(item.ident.to_string()),
                span: item.ident.span(),
            },
            Item::Enum(item) => Self {
                kind: "enum",
                name: Some(item.ident.to_string()),
                span: item.ident.span(),
            },
            Item::ExternCrate(item) => Self {
                kind: "extern crate item",
                name: Some(item.ident.to_string()),
                span: item.ident.span(),
            },
            Item::Fn(item) => Self {
                kind: "function",
                name: Some(item.sig.ident.to_string()),
                span: item.sig.ident.span(),
            },
            Item::ForeignMod(item) => Self {
                kind: "foreign module",
                name: None,
                span: item.span(),
            },
            Item::Impl(item) => Self {
                kind: "impl block",
                name: None,
                span: item.impl_token.span(),
            },
            Item::Macro(item) => Self {
                kind: "macro invocation",
                name: None,
                span: item.span(),
            },
            Item::Mod(item) => Self {
                kind: "module",
                name: Some(item.ident.to_string()),
                span: item.ident.span(),
            },
            Item::Static(item) => Self {
                kind: "static item",
                name: Some(item.ident.to_string()),
                span: item.ident.span(),
            },
            Item::Struct(item) => Self {
                kind: "struct",
                name: Some(item.ident.to_string()),
                span: item.ident.span(),
            },
            Item::Trait(item) => Self {
                kind: "trait",
                name: Some(item.ident.to_string()),
                span: item.ident.span(),
            },
            Item::TraitAlias(item) => Self {
                kind: "trait alias",
                name: Some(item.ident.to_string()),
                span: item.ident.span(),
            },
            Item::Type(item) => Self {
                kind: "type alias",
                name: Some(item.ident.to_string()),
                span: item.ident.span(),
            },
            Item::Union(item) => Self {
                kind: "union",
                name: Some(item.ident.to_string()),
                span: item.ident.span(),
            },
            Item::Use(item) => Self {
                kind: "use item",
                name: None,
                span: item.span(),
            },
            _ => Self {
                kind: "item",
                name: None,
                span: item.span(),
            },
        }
    }
}
