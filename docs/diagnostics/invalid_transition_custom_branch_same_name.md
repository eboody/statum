# Transition Custom Branch Same Name

Status: first-party diagnostic

Source fixture: `../../statum-macros/tests/ui/invalid_transition_custom_branch_same_name.rs`
Compiler-output fixture: `../../statum-macros/tests/ui/invalid_transition_custom_branch_same_name.stderr`

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

enum Branch<A, B> {
    First(A),
    Second(B),
    Pending,
}

#[state]
enum State {
    Draft,
    Accepted,
    Rejected,
}

#[machine]
struct Machine<State> {}

#[transition]
impl Machine<Draft> {
    fn decide(self, accept: bool) -> Branch<Machine<Accepted>, Machine<Rejected>> {
        if accept {
            Branch::First(self.accept())
        } else {
            Branch::Pending
        }
    }

    fn accept(self) -> Machine<Accepted> {
        self.transition()
    }

    fn reject(self) -> Machine<Rejected> {
        self.transition()
    }
}

fn main() {}
```

## Compiler Output

```text
error: Error: transition method `Machine<Draft>::decide` returns an unsupported type.
       Found: `fn decide(self) -> Branch<Machine<Accepted>, Machine<Rejected>>`
       Expected: `fn decide(self) -> Machine<NextState>`
       Reason: expected the impl target machine path directly, a source-backed type alias that expands to it, or that same machine path wrapped in a supported `Option`, `Result`, or `Branch` shape
       Fix: return `Machine<NextState>` directly.
       Primary branch: `Branch<Machine<Accepted>, Machine<Rejected>>`
       Note: Supported wrappers are `::core::option::Option<...>`, `::core::result::Result<..., E>`, and `::statum::Branch<..., ...>`, plus ordinary source-declared type aliases that expand to those shapes.
             Imported aliases, macro-generated aliases, include-generated aliases, ambiguous aliases, and foreign machine paths are still rejected because transition introspection only follows source-backed type aliases it can resolve in-module.
  --> tests/ui/invalid_transition_custom_branch_same_name.rs:31:38
   |
31 |     fn decide(self, accept: bool) -> Branch<Machine<Accepted>, Machine<Rejected>> {
   |                                      ^^^^^^
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

- Found: `fn decide(self) -> Branch<Machine<Accepted>, Machine<Rejected>>`
- Expected: `fn decide(self) -> Machine<NextState>`
- Fix: return `Machine<NextState>` directly.

For first-party diagnostics, this page documents the user-facing Statum message.
For compiler-fallback placeholders, the fixture is still tracked so the guide's
coverage list does not drift from `statum-macros/tests/macro_errors.rs` and the
committed `.stderr` files.
