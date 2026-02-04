# module_path_extractor

A small helper crate for proc macros that need to resolve module paths and module files from a call-site span.

## What it provides
- Call-site file/line discovery via `get_source_info()`.
- Module path resolution via `find_module_path()` or `find_module_path_in_file()`.
- Module root detection via `module_root_from_file()`.
- File-to-module-path helpers via `module_path_from_file()` and `module_path_from_file_with_root()`.
- Module-path-to-file mapping via `module_path_to_file()`.

## Usage
```rust
use module_path_extractor::{
    get_source_info, find_module_path, module_root_from_file,
    find_module_path_in_file, module_path_to_file,
};

let (file, line) = get_source_info().expect("no call-site info");
let module_path = find_module_path(&file, line).expect("no module path");

let root = module_root_from_file(&file);
let module_path_with_root =
    find_module_path_in_file(&file, line, &root).expect("no module path");

let module_file = module_path_to_file(&module_path, &file, &root)
    .expect("module file not found");
```

## Notes
- This crate requires nightly because it uses `proc_macro_span`.
- Module root resolution assumes a standard Cargo layout with `src/`.
