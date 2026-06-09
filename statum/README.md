# statum

`statum` is the public facade crate for Statum, a typed workflow-protocol
framework for Rust.

Use it when you are modeling a concept that moves through distinct states and
you want those states encoded in the type system. In the same spirit as
`Option<T>` and `Result<T, E>`, Statum helps make undesirable states
unrepresentable in code.

This crate re-exports:

- `#[state]`, `#[machine]`, `#[transition]`, and `#[validators]` from
  `statum-macros`.
- Core typestate, transition, validation, and rehydration types from
  `statum-core`.
- Optional graph/presentation surfaces behind the `introspection` feature:
  `MachineIntrospection`, `MachineGraph`, `MachineTransitionRecorder`,
  `MachinePresentation`, and related metadata types.

## Install

```toml
[dependencies]
statum = "0.9.0"
```

Statum targets stable Rust and currently supports Rust `1.93+`. The repository
pins `rust-toolchain.toml` to Rust `1.96.0` for day-to-day development and keeps
`rust-version = "1.93"` in Cargo metadata for the supported minimum.

No default features are enabled. Enable `introspection` when you want generated
machine graphs:

```toml
[dependencies]
statum = { version = "0.9.0", features = ["introspection"] }
```

For the strict graph-metadata authority boundary:

```toml
[dependencies]
statum = { version = "0.9.0", features = ["strict-introspection"] }
```

`strict-introspection` only changes graph metadata generation. Unsupported
transition return shapes are rejected unless the method provides an explicit
`#[introspect(return = ...)]` annotation.

## Mental Model

- `#[state]` defines legal phases.
- `#[machine]` defines durable context shared across phases.
- `#[transition]` defines legal edges.
- `#[validators]` brings dynamic or persisted data back into the typed model.

Statum is storage-agnostic. Database examples are integration patterns, not
built-in adapters.

Use Statum when pressing `.` before and after a phase change should show a
meaningfully different method surface.

Compared with a plain enum, Statum moves legal behavior onto phase-specific
machine types. If `publish()` only exists on `DocumentMachine<Draft>`, code
holding a `DocumentMachine<Published>` cannot call it.

## Minimal Example

```rust
use statum::{machine, state, transition};

#[state]
enum DocumentState {
    Draft,
    Published,
}

#[machine]
struct DocumentMachine<DocumentState> {
    id: i64,
    title: String,
}

#[transition]
impl DocumentMachine<Draft> {
    fn publish(self) -> DocumentMachine<Published> {
        self.transition()
    }
}

fn main() {}
```

`publish()` is only available on `DocumentMachine<Draft>`. Once the document is
published, that transition disappears from the method surface.

## Docs

Start with the repository README and the guided workflow docs:

- Overview: <https://github.com/eboody/statum>
- Start here: <https://github.com/eboody/statum/blob/main/docs/start-here.md>
- Review workflow tutorial:
  <https://github.com/eboody/statum/blob/main/docs/tutorial-review-workflow.md>
- Typed rehydration:
  <https://github.com/eboody/statum/blob/main/docs/persistence-and-validators.md>
- Graph introspection:
  <https://github.com/eboody/statum/blob/main/docs/introspection.md>
