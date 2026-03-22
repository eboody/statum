# statum

`statum` is about representational correctness for workflow and protocol state.
It helps make invalid, undesirable, or not-yet-validated states impossible to
represent as ordinary values.

It applies the same idea as `Option` and `Result`: absence or failure becomes
explicit in the type instead of staying implicit in the program.

This crate re-exports:

- attribute macros: `#[state]`, `#[machine]`, `#[transition]`, `#[validators]`
- runtime types: `statum::Error`, `statum::Result<T>`
- advanced traits: `StateMarker`, `UnitState`, `DataState`, `CanTransition*`
- typed introspection and runtime-join surfaces: `MachineIntrospection`, `MachineGraph`, `MachineTransitionRecorder`, `MachinePresentation`
- projection helpers: `statum::projection`

## Install

```toml
[dependencies]
statum = "0.6.6"
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

# fn main() {}
```

## Docs

- Machine introspection is useful when the machine definition should also drive
  CLI explainers, graph exports, generated docs, branch-strip views, or runtime
  replay/debug tooling. Statum exposes exact transition sites instead of a
  coarse machine-wide state list.
- API docs: <https://docs.rs/statum>
- Repository README: <https://github.com/eboody/statum/blob/main/README.md>
- Coding-agent kit: <https://github.com/eboody/statum/blob/main/docs/agents/README.md>
- Validators guide: <https://github.com/eboody/statum/blob/main/docs/persistence-and-validators.md>
- Examples crate: <https://github.com/eboody/statum/tree/main/statum-examples>
- Repository: <https://github.com/eboody/statum>
