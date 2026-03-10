# statum-macros

`statum-macros` is the proc-macro crate behind Statum.

It provides:
- `#[state]`
- `#[machine]`
- `#[transition]`
- `#[validators]`

Most users should depend on `statum` instead of using this crate directly.

## Install

```toml
[dependencies]
statum-macros = "0.3"
```

## Notes

- This crate is intended for macro expansion support.
- Runtime types such as `Error`/`Result` are in `statum-core` (or re-exported by `statum`).

## Docs

- API docs: <https://docs.rs/statum-macros>
- Repository: <https://github.com/eboody/statum>
