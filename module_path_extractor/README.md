# module_path_extractor

`module_path_extractor` reconstructs Rust module paths from proc-macro call-site
spans and source files.

It is useful when a proc-macro needs to answer questions like:

- what file and line am I expanding from?
- what module path owns this line?
- what file corresponds to `crate::workflow::review`?

This crate is infrastructure for macro systems, registries, and module-aware
diagnostics. It is not a general Rust parser.

## Install

```toml
[dependencies]
module_path_extractor = "0.5.4"
```

## Main APIs

- `get_source_info()`: best-effort `(file_path, line_number)` for the current
  proc-macro call site
- `find_module_path(file_path, line_number)`: resolve the module path owning a
  specific line in a source file
- `find_module_path_in_file(file_path, line_number, module_root)`: same lookup
  when you already know the crate/module root
- `module_path_from_file(...)`: derive a module path from a Rust source file
- `module_path_to_file(...)`: map `crate::foo::bar` back to a source file
- `module_root_from_file(...)`: derive the source root for a file
- `get_pseudo_module_path()`: best-effort legacy helper that falls back to
  `"unknown"`; prefer `get_source_info() + find_module_path()` when you want
  explicit failure handling

## Typical Flow

```rust,ignore
let (file_path, line_number) =
    module_path_extractor::get_source_info().expect("macro call-site source info");

let module_path =
    module_path_extractor::find_module_path(&file_path, line_number).expect("module path");
```

If you already have a file path and want file-to-module or module-to-file
conversion, use the `module_path_from_file(...)`, `module_root_from_file(...)`,
and `module_path_to_file(...)` helpers directly.

## What It Handles

- `lib.rs`, `main.rs`, `mod.rs`, and nested source files
- inline modules
- raw identifier modules like `r#async::r#type`
- stale cache invalidation when source files change

## Intended Use

`module_path_extractor` is a low-level crate. It fits well under tools like
`macro_registry`, or any proc-macro crate that needs deterministic
module-sensitive lookups without shelling out to Cargo or rust-analyzer.

## Docs

- API docs: <https://docs.rs/module_path_extractor>
- Repository: <https://github.com/eboody/statum>
