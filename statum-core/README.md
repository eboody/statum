# statum-core

`statum-core` contains shared runtime types used by the Statum workspace.

Public surface:
- `statum_core::Error`
- `statum_core::Result<T>`

## Install

```toml
[dependencies]
statum-core = "0.3"
```

## Example

```rust
fn ensure_ready(ready: bool) -> statum_core::Result<()> {
    if ready {
        Ok(())
    } else {
        Err(statum_core::Error::InvalidState)
    }
}
```

## Docs

- API docs: <https://docs.rs/statum-core>
- Repository: <https://github.com/eboody/statum>
