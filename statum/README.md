# statum

`statum` provides an ergonomic typestate API for Rust with compile-time transition guarantees.

This crate re-exports:
- Attribute macros: `#[state]`, `#[machine]`, `#[transition]`, `#[validators]`
- Runtime types: `statum::Error`, `statum::Result<T>`
- Builder support: `statum::bon`

## Install

```toml
[dependencies]
statum = "0.3"
```

## Minimal Example

```rust
use statum::{machine, state, transition};

#[state]
enum LightState {
    Off,
    On,
}

#[machine]
struct Light<LightState> {
    name: String,
}

#[transition]
impl Light<Off> {
    fn switch_on(self) -> Light<On> {
        self.transition()
    }
}

#[transition]
impl Light<On> {
    fn switch_off(self) -> Light<Off> {
        self.transition()
    }
}
```

## Docs

- API docs: <https://docs.rs/statum>
- Repository: <https://github.com/eboody/statum>
- Workspace README: <https://github.com/eboody/statum/blob/main/README.md>
