# Transition Foreign Same Leaf Machine

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_transition_foreign_same_leaf_machine.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_transition_foreign_same_leaf_machine.stderr`

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

mod other {
    pub struct Machine<State>(pub core::marker::PhantomData<State>);
}

#[state]
enum State {
    Draft,
    Accepted,
}

#[machine]
struct Machine<State> {}

#[transition]
impl Machine<Draft> {
    fn finish(self) -> other::Machine<Accepted> {
        other::Machine(core::marker::PhantomData)
    }
}

fn main() {}
```

## Compiler Output

```text
error: Error: transition method `Machine<Draft>::finish` returns an unsupported type.
       Found: `fn finish(self) -> other::Machine<Accepted>`
       Expected: `fn finish(self) -> Machine<NextState>`
       Reason: expected the impl target machine path directly, a source-backed type alias that expands to it, or that same machine path wrapped in a supported `Option`, `Result`, or `Branch` shape
       Fix: return `Machine<NextState>` directly.
       Primary branch: `other::Machine<Accepted>`
       Note: Supported wrappers are `::core::option::Option<...>`, `::core::result::Result<..., E>`, and `::statum::Branch<..., ...>`, plus ordinary source-declared type aliases that expand to those shapes.
             Imported aliases, macro-generated aliases, include-generated aliases, ambiguous aliases, and foreign machine paths are still rejected because transition introspection only follows source-backed type aliases it can resolve in-module.
  --> tests/ui/invalid_transition_foreign_same_leaf_machine.rs:28:24
   |
28 |     fn finish(self) -> other::Machine<Accepted> {
   |                        ^^^^^
```

## Corrected Example

```rust
use statum::{machine, state, transition};

#[state]
enum WorkflowState {
    Draft,
    Review,
}

#[machine]
struct WorkflowMachine<WorkflowState> {}

#[transition]
impl WorkflowMachine<Draft> {
    fn submit(self) -> WorkflowMachine<Review> {
        self.transition_to()
    }
}
```

## Explanation

- Found: `fn finish(self) -> other::Machine<Accepted>`
- Expected: `fn finish(self) -> Machine<NextState>`
- Fix: return `Machine<NextState>` directly.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
