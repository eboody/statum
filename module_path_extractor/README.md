# module_path_extractor

`module_path_extractor` derives module paths and source info for proc-macro call sites.

Key APIs:
- `get_source_info()`
- `find_module_path()`
- `module_path_from_file()`
- `module_path_to_file()`

## Install

```toml
[dependencies]
module_path_extractor = "0.5"
```

## Intended Use

This crate is for proc-macro infrastructure where module resolution must be reconstructed from file paths and spans.

## Docs

- API docs: <https://docs.rs/module_path_extractor>
- Repository: <https://github.com/eboody/statum>
