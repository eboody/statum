# statum

`statum` is about representational correctness when a value's phase should
change what methods are legally available on that value. It helps make invalid,
undesirable, or not-yet-validated states impossible to represent as ordinary
values.

It applies the same idea as `Option` and `Result`: absence or failure becomes
explicit in the type instead of staying implicit in the program.

This crate re-exports:

- attribute macros: `#[state]`, `#[machine]`, `#[transition]`, `#[validators]`
- runtime types: `statum::Error`, `statum::Result<T>`
- advanced traits: `StateMarker`, `UnitState`, `DataState`, `CanTransition*`
- optional typed introspection and runtime-join surfaces behind the `introspection` feature: `MachineIntrospection`, `MachineGraph`, `MachineTransitionRecorder`, `MachinePresentation`
- projection helpers: `statum::projection`

## Install

```toml
[dependencies]
statum = "0.8.10"
```

Statum targets stable Rust and currently supports Rust `1.93+`.

For the strict graph-metadata authority boundary, enable:

```toml
[dependencies]
statum = { version = "0.8.10", features = ["strict-introspection"] }
```

The repository tracks stable Rust for daily development and checks Rust `1.93.1`
in CI as the MSRV job.

## Mental Model

- Use `statum` when pressing `.` before and after a phase change should show a
  meaningfully different method surface.
- Durable workflows and protocols are one strong fit. Staged validation,
  resolution, and build surfaces are another.
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

- Enable the `introspection` feature when the machine definition should also drive
  CLI explainers, graph exports, generated docs, branch-strip views, or runtime
  replay/debug tooling. That feature emits the generated `StateId`,
  `TransitionId`, `GRAPH`, `PRESENTATION`, and `linkme` inventory surface; the
  default feature set keeps those out of generated machines. Its observation
  point is the macro-validated semantic model: locally readable state/machine
  items plus transition signatures and explicit `#[introspect(return = ...)]`
  overrides.
- API docs: <https://docs.rs/statum>
- Repository README: <https://github.com/eboody/statum/blob/main/README.md>
- Validators guide: <https://github.com/eboody/statum/blob/main/docs/persistence-and-validators.md>
- Examples crate: <https://github.com/eboody/statum/tree/main/statum-examples>
- Repository: <https://github.com/eboody/statum>
