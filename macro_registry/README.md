# macro_registry

`macro_registry` provides reusable proc-macro registry and source-analysis utilities.

Primary modules:
- `analysis`: cached file analysis (`enum`/`struct` discovery)
- `callsite`: source/module path helpers at macro call sites
- `registry`: generic static registry loader and lookup infrastructure

## Install

```toml
[dependencies]
macro_registry = "0.5"
```

## Intended Use

This crate is mainly useful for proc-macro authors building multi-macro systems that need deterministic metadata lookup across files and modules.

## Docs

- API docs: <https://docs.rs/macro_registry>
- Repository: <https://github.com/eboody/statum>
