# statum-macros

`statum-macros` is the proc-macro crate behind Statum.

It provides:

- `#[state]`
- `#[machine]`
- `#[transition]`
- `#[validators]`

Most users should depend on `statum` instead of using this crate directly.

`strict-introspection` is an optional feature. The public `statum` crate forwards
its `strict-introspection` feature to this crate, where unsupported graph return
shapes are rejected unless the transition provides an explicit
`#[introspect(return = ...)]` annotation.

## Install

```toml
[dependencies]
statum-macros = "0.8.10"
```

## Notes

- This crate is intended for macro expansion support and proc-macro internals.
- Runtime types such as `Error`/`Result` are in `statum-core` (or re-exported by `statum`).
- The public-facing macro docs are best read through `statum` on docs.rs.

## Docs

- API docs: <https://docs.rs/statum-macros>
- End-user docs: <https://docs.rs/statum>
- Repository: <https://github.com/eboody/statum>
