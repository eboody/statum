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
statum-macros = "0.5"
```

## Notes

- This crate is intended for macro expansion support and proc-macro internals.
- Runtime types such as `Error`/`Result` are in `statum-core` (or re-exported by `statum`).
- The public-facing macro docs are best read through `statum` on docs.rs.

## Docs

- API docs: <https://docs.rs/statum-macros>
- End-user docs: <https://docs.rs/statum>
- Repository: <https://github.com/eboody/statum>
