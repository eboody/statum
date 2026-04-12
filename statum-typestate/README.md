# statum-typestate

`statum-typestate` is the small Statum package for teams that only want the
core typestate path.

The package name is `statum-typestate`, but it exports the crate name
`statum`, so downstream code still writes `use statum::{machine, state,
transition};`.

## Scope

Included:

- `#[state]`
- `#[machine]`
- `#[transition]`
- `StateMarker`, `UnitState`, `DataState`
- `CanTransitionTo`, `CanTransitionWith`, `CanTransitionMap`
- `Branch`, `Attested`, `Error`, `Result`

Not part of the documented end-user surface:

- `#[validators]`
- machine introspection and graph metadata
- `#[machine_ref]`
- projection helpers

If you need those surfaces as supported entry points, depend on `statum`
instead.

## Install

```toml
[dependencies]
statum-typestate = "0.7.1"
```

## Quick Start

```rust
use statum::{machine, state, transition};

#[state]
enum ReviewState {
    Draft,
    Submitted(String),
    Approved,
}

#[machine]
struct Review<ReviewState> {
    id: u64,
}

#[transition]
impl Review<Draft> {
    fn submit(self, title: String) -> Review<Submitted> {
        self.transition_with(title)
    }
}

#[transition]
impl Review<Submitted> {
    fn approve(self) -> Review<Approved> {
        self.transition()
    }
}

fn main() {
    let review = Review::<Draft>::builder().id(7).build();
    let submitted = review.submit("RFC".to_owned());
    let _approved = submitted.approve();
}
```

## Audit Surface

This package is intentionally thin. It is a wrapper around:

- `statum-core`
- `statum-macros` with default features disabled
- this crate, which only re-exports the typestate-only surface

That gives adopters a concrete boundary to audit when they do not want the
full validators, introspection, and tooling stack.

For mixed dependency graphs that also pull in the full `statum` package, this
crate keeps a hidden compatibility re-export layer so Cargo feature unification
does not break macro expansion. The documented surface above remains the
intended package boundary.

## Docs

- API docs: <https://docs.rs/statum-typestate>
- Full package: <https://docs.rs/statum>
- Repository: <https://github.com/eboody/statum>
