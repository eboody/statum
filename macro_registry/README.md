# macro_registry

`macro_registry` is a small proc-macro infrastructure crate for three related
jobs:

1. inspect a Rust source file for enums and structs
2. query those items by module and attribute
3. cache typed metadata behind a registry with explicit lookup results

It is useful when multiple macros need to discover and reuse the same items in
the same file or module without re-implementing lookup and caching logic.

## Install

```toml
[dependencies]
macro_registry = "0.5.4"
```

## Modules

- `analysis`: cached file analysis for enum and struct discovery
- `callsite`: current source and module helpers for proc-macro call sites
- `query`: module-aware item search for diagnostics and pre-resolution
- `registry`: typed registry loading for cached metadata

## Typical Flow

Most callers use the crate in this order:

1. get a file path and line number from the macro call site
2. query the current file for candidate items in one module
3. load and cache the typed metadata you actually want

Simple query example:

```rust
use macro_registry::query::{ItemKind, candidates_in_module};

# let file_path = "/tmp/lib.rs";
let machines = candidates_in_module(
    file_path,
    "crate::workflow",
    ItemKind::Struct,
    Some("machine"),
);
```

If you need cached typed lookup instead of a flat candidate list, use the
registry layer:

```rust,ignore
use macro_registry::analysis::{FileAnalysis, StructEntry};
use macro_registry::registry::{
    LookupMode, NamedRegistryDomain, RegistryDomain, RegistryKey, RegistryValue,
    SourceContext, StaticRegistry, try_ensure_loaded_by_name_from_source,
};

#[derive(Clone, Eq, Hash, PartialEq)]
struct MachinePath(String);

impl AsRef<str> for MachinePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl RegistryKey for MachinePath {
    fn from_module_path(module_path: String) -> Self {
        Self(module_path)
    }
}

#[derive(Clone)]
struct MachineMeta {
    name: String,
    file_path: Option<String>,
}

impl RegistryValue for MachineMeta {
    fn file_path(&self) -> Option<&str> {
        self.file_path.as_deref()
    }

    fn set_file_path(&mut self, file_path: String) {
        self.file_path = Some(file_path);
    }
}

struct MachineDomain;

impl RegistryDomain for MachineDomain {
    type Key = MachinePath;
    type Value = MachineMeta;
    type Entry = StructEntry;

    fn entries(analysis: &FileAnalysis) -> &[Self::Entry] {
        &analysis.structs
    }

    fn entry_line(entry: &Self::Entry) -> usize {
        entry.line_number
    }

    fn build_value(entry: &Self::Entry, _module: &Self::Key) -> Option<Self::Value> {
        Some(MachineMeta {
            name: entry.item.ident.to_string(),
            file_path: None,
        })
    }
}

impl NamedRegistryDomain for MachineDomain {
    fn entry_name(entry: &Self::Entry) -> String {
        entry.item.ident.to_string()
    }

    fn value_name(value: &Self::Value) -> String {
        value.name.clone()
    }
}

static MACHINES: StaticRegistry<MachinePath, MachineMeta> = StaticRegistry::new();

let source = SourceContext::new(file_path, line_number);
let loaded = try_ensure_loaded_by_name_from_source::<MachineDomain>(
    &MACHINES,
    LookupMode::Exact(MachinePath("crate::workflow".into())),
    "TaskMachine",
    &source,
)?;
```

## Why Use `registry`

The registry layer is the part that turns a lookup from "maybe found something"
into a deterministic contract.

- `LookupMode::Exact(...)`: require one module
- `LookupMode::AnyModule`: search without seeding a fake sentinel key
- `LookupFailure::NotFound`: nothing matched
- `LookupFailure::Ambiguous`: too many candidates matched
- `SourceContext`: run the same lookup logic against an explicit file and line

If you only need candidate lists for diagnostics, `query` is often enough. If
you are building a multi-macro system that reuses typed metadata, use
`registry`.

## Intended Use

This crate is for proc-macro authors and macro infrastructure, not for general
application code. In the Statum workspace it powers the macro metadata layer,
but the API is intentionally generic enough for other module-aware proc-macro
systems.

## Docs

- API docs: <https://docs.rs/macro_registry>
- Repository: <https://github.com/eboody/statum>
