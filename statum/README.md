# statum

`statum` provides compile-time verified typestate workflows for Rust.

This crate re-exports:

- attribute macros: `#[state]`, `#[machine]`, `#[transition]`, `#[validators]`
- runtime types: `statum::Error`, `statum::Result<T>`
- advanced traits: `StateMarker`, `UnitState`, `DataState`, `CanTransition*`
- projection helpers: `statum::projection`

## Install

```toml
[dependencies]
statum = "0.6.0"
```

Statum targets stable Rust and currently supports Rust `1.93+`.

## Mental Model

- `#[state]` defines the legal phases
- `#[machine]` defines the durable context
- `#[transition]` defines the legal edges
- `#[validators]` rebuilds typed machines from stored data

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
- Repository README: <https://github.com/eboody/statum/blob/main/README.md>
- Coding-agent kit: <https://github.com/eboody/statum/blob/main/docs/agents/README.md>
- Validators guide: <https://github.com/eboody/statum/blob/main/docs/persistence-and-validators.md>
- Examples crate: <https://github.com/eboody/statum/tree/main/statum-examples>
- Repository: <https://github.com/eboody/statum>
