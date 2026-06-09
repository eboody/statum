# Default Introspection Surface

Status: tracked compiler-fallback placeholder

Source fixture: `../../statum-macros/tests/ui/invalid_default_introspection_surface.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_default_introspection_surface.stderr`

## Broken Example

```rust
#![allow(unused_imports)]
extern crate self as statum;
pub use statum_core::__private;
pub use statum_core::TransitionInventory;
pub use statum_core::{
    CanTransitionMap, CanTransitionTo, CanTransitionWith, DataState, Error, MachineDescriptor,
    MachineGraph, MachineIntrospection, MachineStateIdentity, RebuildAttempt, RebuildReport,
    StateDescriptor, StateMarker, TransitionDescriptor, UnitState,
};

use statum_macros::{machine, state, transition};

#[state]
enum FlowState {
    Draft,
    Review,
}

#[machine]
struct Flow<FlowState> {}

#[transition]
impl Flow<Draft> {
    fn submit(self) -> Flow<Review> {
        self.transition()
    }
}

fn main() {
    let _graph = flow::GRAPH;
    let _state = flow::StateId::Draft;
    let _transition = Flow::<Draft>::SUBMIT;
}
```

## Compiler Output

```text
error[E0433]: cannot find `StateId` in `flow`
  --> tests/ui/invalid_default_introspection_surface.rs:31:24
   |
31 |     let _state = flow::StateId::Draft;
   |                        ^^^^^^^ could not find `StateId` in `flow`
   |
help: a type alias with a similar name exists
   |
31 -     let _state = flow::StateId::Draft;
31 +     let _state = flow::State::Draft;
   |

error[E0425]: cannot find value `GRAPH` in module `flow`
  --> tests/ui/invalid_default_introspection_surface.rs:30:24
   |
30 |     let _graph = flow::GRAPH;
   |                        ^^^^^ not found in `flow`

error[E0599]: no associated item named `SUBMIT` found for struct `Flow<FlowState>` in the current scope
  --> tests/ui/invalid_default_introspection_surface.rs:32:38
   |
19 | #[machine]
   | ---------- associated item `SUBMIT` not found for this struct
...
32 |     let _transition = Flow::<Draft>::SUBMIT;
   |                                      ^^^^^^ associated item not found in `Flow<Draft>`
```

## Corrected Example

```toml
# Cargo.toml
statum = { version = "...", features = ["introspection"] }
```

```rust
// With the `introspection` feature enabled, these generated items are part of
// the public machine surface.
let _graph = flow::GRAPH;
let _state = flow::StateId::Draft;
let _transition = Flow::<Draft>::SUBMIT;
```

Default builds intentionally expose only the feature-free state/machine API; do
not depend on graph constants, state IDs, or transition constants unless the
crate feature says that introspection is enabled.
