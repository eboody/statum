use core::fmt;
use std::fs;
use std::path::{Path as FsPath, PathBuf};
use std::time::UNIX_EPOCH;

use macro_registry::callsite::current_source_info;
use macro_registry::registry;
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::spanned::Spanned;
use syn::{Attribute, Item, Path as SynPath};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceFingerprint {
    len: u64,
    modified_ns: Option<u128>,
}

fn normalize_path(path: &FsPath) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }

    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

pub(crate) fn source_file_fingerprint(file_path: &str) -> Option<SourceFingerprint> {
    let metadata = fs::metadata(normalize_path(FsPath::new(file_path))).ok()?;
    let modified_ns = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos());

    Some(SourceFingerprint {
        len: metadata.len(),
        modified_ns,
    })
}

pub(crate) fn crate_root_for_file(file_path: &str) -> Option<String> {
    let mut path = normalize_path(FsPath::new(file_path));
    if path.is_file() {
        path = path.parent()?.to_path_buf();
    }

    let mut cursor = Some(path.as_path());
    while let Some(dir) = cursor {
        if dir.join("Cargo.toml").is_file() {
            return Some(dir.to_string_lossy().into_owned());
        }
        cursor = dir.parent();
    }

    None
}

pub(crate) fn current_crate_root() -> Option<String> {
    let (file_path, _) = current_source_info()?;
    crate_root_for_file(&file_path)
}

pub(crate) fn extract_derives(attr: &Attribute) -> Option<Vec<String>> {
    if !attr.path().is_ident("derive") {
        return None;
    }

    attr.meta
        .require_list()
        .ok()?
        .parse_args_with(syn::punctuated::Punctuated::<SynPath, syn::Token![,]>::parse_terminated)
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::thread;
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{crate_root_for_file, source_file_fingerprint};

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("statum_syntax_{label}_{nanos}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, contents).expect("write file");
    }

    #[test]
    fn crate_root_for_file_walks_up_to_manifest_dir() {
        let crate_dir = unique_temp_dir("crate_root");
        let src = crate_dir.join("src");
        let nested = crate_dir.join("tests").join("ui");
        let lib = src.join("lib.rs");
        let fixture = nested.join("fixture.rs");

        write_file(
            &crate_dir.join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        );
        write_file(&lib, "pub fn marker() {}\n");
        write_file(&fixture, "fn main() {}\n");

        assert_eq!(
            crate_root_for_file(&lib.to_string_lossy()).as_deref(),
            Some(crate_dir.to_string_lossy().as_ref())
        );
        assert_eq!(
            crate_root_for_file(&fixture.to_string_lossy()).as_deref(),
            Some(crate_dir.to_string_lossy().as_ref())
        );

        let _ = fs::remove_dir_all(crate_dir);
    }

    #[test]
    fn source_file_fingerprint_tracks_file_changes() {
        let crate_dir = unique_temp_dir("fingerprint");
        let file = crate_dir.join("src").join("lib.rs");
        write_file(&file, "pub fn marker() {}\n");
        let before = source_file_fingerprint(&file.to_string_lossy()).expect("before fingerprint");

        thread::sleep(Duration::from_millis(5));
        write_file(&file, "pub fn marker() {}\npub fn changed() {}\n");
        let after = source_file_fingerprint(&file.to_string_lossy()).expect("after fingerprint");

        assert_ne!(before, after);

        let _ = fs::remove_dir_all(crate_dir);
    }
}
