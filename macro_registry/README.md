# macro_registry

`macro_registry` is shared infrastructure for proc-macro crates that need:

- call-site source/module-path resolution,
- file-level AST analysis cached per source file, and
- lightweight static registries keyed by module path.

It is used by `statum-macros` to resolve `#[state]` and `#[machine]` declarations from
macro call sites without duplicating registry/cache logic.

## Modules

- `callsite`: wrappers around `module_path_extractor` for source info and module path lookup.
- `analysis`: memoized `syn::File` analysis for enums/structs in a source file.
- `registry`: generic registry traits and loading helpers.

## Example

```rust,no_run
use macro_registry::callsite::{current_module_path, current_source_info};

let module_path = current_module_path();
let source = current_source_info();
println!("module={module_path} source={source:?}");
```

## MSRV

Follows the `statum` workspace toolchain.
